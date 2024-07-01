mod errors;
mod cache;
mod config;

use std::{collections::{btree_map::Entry, BTreeMap}, fs::{read_to_string, File}, io::BufReader, str::{from_utf8, FromStr}, sync::Arc};

use log::info;
use rustls::ServerConfig;
use rustls_pemfile::{certs, pkcs8_private_keys};
use actix_web::{http::StatusCode, web::{self, Data}, App, HttpResponse, HttpServer};
use anyhow::{anyhow, Context};
use cache::{cache::{BinaryCache, Cache}, mem_cache::BinaryMemCache};
use clap::Parser;
use errors::{InvalidHeaderNameError, InvalidHeaderValueError, MyCorruptedDBError, MyResult};
use reqwest::ClientBuilder;
use ic_agent::Agent;
use candid::{Decode, Encode};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use anyhow::bail;

use crate::config::Config;

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long="config", default_value="config.toml")]
    config_file: String,
}

struct State {
    client: reqwest::Client,
    agent: Option<Agent>,
    additional_response_headers: Arc<Vec<(http_for_actix::HeaderName, http_for_actix::HeaderValue)>>,
    response_headers_to_remove: Arc<Vec<http_for_actix::HeaderName>>,
}

// Two similar functions with different data types follow:

fn serialize_http_request(request: &actix_web::HttpRequest, url: &str, bytes: &actix_web::web::Bytes) -> anyhow::Result<Vec<u8>> {
    // Actix convert headers to lowercase.
    let mut headers = BTreeMap::new();
    for (k, v) in request.headers().into_iter() { // lexigraphical order
        let entry = headers.entry(k.as_str());
        let v_str = v.to_str()?;
        match entry {
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(vec![v_str]);
            }
            Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut().push(v_str);
            }
        }
    }
    let headers_list = headers.into_iter()
        .map(|(k, v)| k.to_string() + "\t" + &v.join("\t"))
        .collect::<Vec<_>>();
    let headers_joined = headers_list.into_iter().reduce(|a, b| a + "\r" + &b);
    let headers_joined = headers_joined.unwrap_or_else(|| "".to_string());
    let header_part = request.method().as_str().to_owned() + "\n" + url + "\n" + &headers_joined;

    Ok([header_part.as_bytes(), b"\n", bytes.to_vec().as_slice()].concat())
}

async fn serialize_http_response(response: reqwest::Response) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let headers_list = response.headers().into_iter()
        .map(|(k, v)| -> anyhow::Result<String> {
            Ok(k.to_string() + "\t" + v.to_str()?)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let headers_joined = headers_list.into_iter().reduce(|a, b| a + "\r" + &b);
    let headers_joined = headers_joined.unwrap_or_else(|| "".to_string());
    let header_part = response.status().as_u16().to_string() + "\n" + &headers_joined;

    let bytes = response.bytes().await?;
    Ok(([header_part.as_bytes(), b"\n", &bytes].concat(), bytes.to_vec()))
}

fn deserialize_http_response(data: &[u8]) -> anyhow::Result<actix_web::HttpResponse<Vec<u8>>> {
    let mut iter1 = data.splitn(3, |&c| c == b'\n');
    let status_code_bytes = iter1.next().ok_or_else(|| MyCorruptedDBError::default())?;
    let headers_bytes = iter1.next().ok_or_else(|| MyCorruptedDBError::default())?;
    let body = iter1.next().ok_or_else(|| MyCorruptedDBError::default())?;

    let status_code: u16 = str::parse(from_utf8(status_code_bytes)?)?;
    let mut response = actix_web::HttpResponse::with_body(
        StatusCode::from_u16(status_code)?, Vec::from(body));

    let headers = response.headers_mut();
    for header_str in headers_bytes.split(|&c| c == b'\r') {
        let mut iter2 = header_str.splitn(2, |&c| c == b'\t');
        let k = iter2.next().ok_or_else(|| MyCorruptedDBError::default())?;
        let v = iter2.next().ok_or_else(|| MyCorruptedDBError::default())?;
        headers.append(http_for_actix::HeaderName::from_bytes(k)?, http_for_actix::HeaderValue::from_bytes(v)?);
    }

    Ok(response)
}

fn obtain_upstream_base_url(req: &actix_web::HttpRequest) -> anyhow::Result<String> {
    let host = req.headers().get("host")
        .ok_or_else(|| anyhow!("Missing Host: header"))?
        .to_str()?;
    Ok("https://".to_string() + host)
}

async fn prepare_request(req: &actix_web::HttpRequest, url: String, body: &web::Bytes, config: &Data<Config>, state: &Data<State>)
    -> MyResult<(reqwest::Request, String)>
{
    let uri = http::Uri::from_str(url.as_str())?;
    let host = uri.host().ok_or_else(|| anyhow!("no host"))?;
    // TODO: a wrong preliminary optimization below:
    let request_headers = req.headers().into_iter()
        .map(|h| (h.0.clone(), h.1.clone()))
        .filter(|h|
            !config.request_headers.remove.contains(&h.0.to_string()) ||
                h.0 == http_for_actix::HeaderName::from_static("host"))
        .filter(|h|
            if let Some(headers) = config.as_ref().request_headers.remove_per_host.get(host) {
                !headers.contains(&h.0.to_string())
            } else {
                true
            }
        )
        .chain(
            state.as_ref().additional_response_headers.iter().map(|h| (h.0.clone(), h.1.clone()))
        )
        .chain({
            if let Some(headers) = config.as_ref().request_headers.add_per_host.get(host) {
                headers.into_iter().map(|(k, v)| -> MyResult<(http_for_actix::HeaderName, http_for_actix::HeaderValue)> {
                    Ok(
                        (
                            http_for_actix::HeaderName::from_str(k.as_str()).map_err(|_| InvalidHeaderNameError::default())?,
                            http_for_actix::HeaderValue::from_str(v.as_str()).map_err(|_| InvalidHeaderValueError::default())?,
                        )
                    )
                }).collect::<MyResult<Vec<_>>>()?
            } else {
                vec![]
            }.into_iter()
        });
    
    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())?;
    let headers = http::HeaderMap::from_iter(
        request_headers
            .map(|h| -> MyResult<_> {
                Ok((
                    http::HeaderName::from_str(h.0.as_str()).map_err(|_| InvalidHeaderNameError::default())?,
                    http::HeaderValue::from_str(h.1.to_str()?).map_err(|_| InvalidHeaderValueError::default())?,
                ))
            })
            .into_iter()
            .collect::<MyResult<Vec<_>>>()?
    );
    let builder = state.client.request(method, url).headers(headers).body(Vec::from(body.as_ref()));
    Ok((builder.build()?, host.to_string()))
}

async fn proxy(
    req: actix_web::HttpRequest,
    body: web::Bytes,
    config: Data<Config>,
    cache: Data<Arc<tokio::sync::Mutex<Box<BinaryMemCache>>>>,
    state: Data<State>, 
)
    -> MyResult<actix_web::HttpResponse<Vec<u8>>>
{
    let path = req.uri().path_and_query().ok_or(anyhow!("can't get path and query"))?.as_str();
    info!("Joining proxy received a request to {}", path);
    // First level of defence: X-JoinProxy-Key can be stolen by an IC replica owner:
    if let Some(our_secret) = &config.our_secret {
        let passed_key = req.headers()
            .get("x-joinproxy-key")
            .map(|v| v.to_str().map_err(|_| anyhow!("Cannot read header X-JoinProxy-Key")))
            .transpose()?;
        if passed_key != Some(&("Bearer ".to_string() + &our_secret)) {
            return Ok(HttpResponse::with_body(StatusCode::NETWORK_AUTHENTICATION_REQUIRED, Vec::new()));
        }
    }

    // TODO: Test that it works for paths like `/xx?` with question sign but without arguments.
    // TODO: Check that https://example.com and https://example.com/ are exchangeable.
    let serialized_request = serialize_http_request(&req, path, &body)?;
    let actix_request_hash = Sha256::digest(serialized_request.as_slice());

    let mut cache = (***cache).lock().await;

    // We lock during the time of downloading from upstream to prevent duplicate requests with identical data.
    let mut cache_lock = cache.lock(&Vec::from(actix_request_hash.as_slice())).await?;

    if let Some(serialized_response) = (*cache_lock).inner().await
    {
        std::mem::drop(cache_lock);
        info!("Cache hit.");

        let mut response = deserialize_http_response(serialized_response.as_slice())?;
        if config.response_headers.show_hit_miss {
            response.headers_mut().append(
                http_for_actix::HeaderName::from_str("X-JoinProxy-Response").unwrap(),
                http_for_actix::HeaderValue::from_str("Hit").unwrap(),
            );
        }
        Ok(response)
    } else {
        info!("Cache miss.");

        // Second level of defence: Ask back the calling canister.
        // Do it only once per outcall (our response content isn't secure anyway).
        if let (Some(agent), Some(callback)) = (&state.agent, &config.callback) {
            info!("Callback...");
            let res = agent.update(&callback.canister, &callback.func)
                .with_arg(Encode!(&actix_request_hash.as_slice())?).call_and_wait().await;
            match res {
                Ok(res) => {
                    Decode!(res.as_slice()).context("Callback decode")?; // checking for errors
                    info!("Callback OK.");
                }
                Err(e) => {
                    info!("Callback failed: {e}");
                    Err(e)?;
                }
            }
        }

        let base_url = obtain_upstream_base_url(&req)?;
        let (reqwest, host) = prepare_request(&req, base_url + path, &body, &config, &state).await?;
        let reqwest_response = state.client.execute(reqwest).await?;
        info!("Upstream status: {}", reqwest_response.status());
        let status = reqwest_response.status().as_u16();

        let mut actix_response = actix_web::HttpResponse::new(
            StatusCode::from_u16(status)?);
        let headers = actix_response.headers_mut();
        for (k, v) in reqwest_response.headers() {
            headers.append(
                http_for_actix::HeaderName::from_str(k.as_str()).map_err(|_| InvalidHeaderNameError::default())?,
                http_for_actix::HeaderValue::from_str(v.to_str()?).map_err(|_| InvalidHeaderValueError::default())?,
            );
        }

        // We retrieved the response, immediately set and release the cache:
        let (cached, bytes) = serialize_http_response(reqwest_response).await?;
        (*cache_lock).set(Some(cached)).await;
        std::mem::drop(cache_lock);

        if config.response_headers.show_hit_miss {
            headers.append(
                http_for_actix::HeaderName::from_str("X-JoinProxy-Response").unwrap(),
                http_for_actix::HeaderValue::from_str("Miss").unwrap(),
            );
        }
        if config.response_headers.add_forwarded_from_header {
            if let Some(addr) = req.head().peer_addr {
                headers.append(
                    http_for_actix::HeaderName::from_str("X-Forwarded-For").unwrap(),
                    http_for_actix::HeaderValue::from_str(&addr.ip().to_string()).unwrap(),
                );
            }
        }
        for k in state.response_headers_to_remove.iter() {
            headers.remove(k);
        }
        if let Some(remove) = config.response_headers.remove_per_host.get(&host) {
            for k in remove.into_iter() {
                headers.remove(k);
            }
        }
        for (k, v) in config.response_headers.add.iter() {
            headers.append(
                http_for_actix::HeaderName::from_str(k).map_err(|_| InvalidHeaderNameError::default())?,
                http_for_actix::HeaderValue::from_str(&v).map_err(|_| InvalidHeaderValueError::default())?
            );
        }
        if let Some(add) = config.response_headers.add_per_host.get(&host) {
            for (k, v) in add.into_iter() {
                headers.append(
                    http_for_actix::HeaderName::from_str(k).map_err(|_| InvalidHeaderNameError::default())?,
                    http_for_actix::HeaderValue::from_str(&v).map_err(|_| InvalidHeaderValueError::default())?
                );
            }
        }

        // let response_body: Vec<u8> = Vec::from(bytes);
        Ok(actix_response.set_body(bytes))
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();
    let config_string = read_to_string(&args.config_file)
        .map_err(|e| anyhow!("Cannot read config file {}: {}", args.config_file, e))?;
    let mut config: Config = toml::from_str(&config_string)
        .map_err(|e| anyhow!("Cannot read config file {}: {}", args.config_file, e))?;

    if let Some(callback) = &mut config.callback {
        if callback.ic_url.is_none() && callback.ic_local {
            callback.ic_url = Some("http://localhost:8000".to_string())
        }
    }

    let server_url = config.serve.host.clone() + ":" + config.serve.port.to_string().as_str();

    let cache =
        Arc::new(Mutex::new(Box::<BinaryCache>::from(Box::new(BinaryMemCache::new(config.cache.cache_timeout)))));

    let additional_response_headers = &config.request_headers.add;
    let additional_response_headers = additional_response_headers.into_iter().map(
        |v| -> MyResult<_> {
            Ok((
                http_for_actix::HeaderName::from_str(&v.0).map_err(|_| InvalidHeaderNameError::default())?,
                http_for_actix::HeaderValue::from_str(&v.1).map_err(|_| InvalidHeaderValueError::default())?
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let additional_response_headers = Arc::new(additional_response_headers);

    //  http://tools.ietf.org/html/rfc2616#section-13.5.1
    let hop_by_hop = ["connection", "keep-alive", "te", "trailers", "transfer-encoding", "upgrade"];
    let response_headers_to_remove =
        hop_by_hop.into_iter()
            .chain(config.response_headers.remove.iter().map(|s| s.as_str()))
            .map(|h| http_for_actix::HeaderName::from_str(h).map_err(|_| InvalidHeaderNameError::default().into()));
    let response_headers_to_remove = response_headers_to_remove.collect::<MyResult<Vec<_>>>()?;
    let response_headers_to_remove = Arc::new(response_headers_to_remove);

    let agent = {
        if let Some(callback) = &config.callback {
            let mut builder = Agent::builder();
            if let Some(ic_url) = &callback.ic_url {
                builder = builder.with_url(ic_url);
            }
            let agent = builder.build()?;
            if callback.ic_local {
                agent.fetch_root_key().await?;
            }
            Some(agent)
        } else {
            None
        }
    };

    let (cert_file, key_file) = (config.serve.cert_file.clone(), config.serve.key_file.clone());
    let is_https = config.serve.https;
    let server = HttpServer::new(move || {
        let mut builder = ClientBuilder::new();
        if let Some(t) = config.upstream_timeouts.connect_timeout {
            builder = builder.connect_timeout(t);
        }
        if let Some(t) = config.upstream_timeouts.read_timeout {
            builder = builder.connect_timeout(t);
        }
        if let Some(t) = config.upstream_timeouts.total_timeout {
            builder = builder.timeout(t);
        }
        let state = State {
            client: builder.build().unwrap(),
            additional_response_headers: additional_response_headers.clone(),
            response_headers_to_remove: response_headers_to_remove.clone(),
            agent: agent.clone(),
        };
        App::new().service(
            web::scope("")
            .app_data(Data::new(config.clone()))
            .app_data(Data::new(state))
            .app_data(Data::new(cache.clone()))
                .route("/{_:.*}", web::route().to(proxy))
        )
    });
    info!("Starting Proxy at {} (https={})", server_url, is_https);
    if is_https {
        if let (Some(cert_file), Some(key_file)) = (cert_file, key_file) {
            let cert_file = &mut BufReader::new(File::open(cert_file).context("Can't read HTTPS cert.")?);
            let key_file = &mut BufReader::new(File::open(key_file).context("Can't read HTTPS key.")?);
            let cert_chain = certs(cert_file).collect::<Result<Vec<_>, _>>()
                .context("Can't parse HTTPS certs chain.")?;
            let key = pkcs8_private_keys(key_file)
                .next().transpose()?.ok_or(anyhow!("No private key in the file."))?;
            server.bind_rustls_0_23(
                server_url,
                ServerConfig::builder().with_no_client_auth()
                    .with_single_cert(cert_chain, rustls::pki_types::PrivateKeyDer::Pkcs8(key))?
            )
        } else {
            bail!("No SSL certificate or key in config");
        }
    } else {
        server.bind(server_url)
    }?
        .run()
        .await.map_err(|e| e.into())
}
