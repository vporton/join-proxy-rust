mod errors;
mod cache;
mod config;

use std::{collections::{btree_map::Entry, BTreeMap}, fs::{read_to_string, File}, io::BufReader, str::{from_utf8, FromStr}, sync::Arc, time::Instant};

use log::info;
use rustls::ServerConfig;
use rustls_pemfile::{certs, pkcs8_private_keys};
use actix_web::{http::StatusCode, web::{self, Data}, App, HttpResponse, HttpServer};
use anyhow::{anyhow, Context};
use cache::{cache::Cache, mem_cache::BinaryMemCache};
use clap::Parser;
use errors::{InvalidHeaderNameError, InvalidHeaderValueError, MyCorruptedDBError, MyResult};
use reqwest::ClientBuilder;
use ic_agent::Agent;
use candid::{Decode, Encode};
use sha2::{Digest, Sha256};
use tokio::{sync::Mutex, time::sleep};
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
    // FIXME: Should convert headers to lowercase?
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

fn serialize_http_response(response: &reqwest::Response, bytes: bytes::Bytes) -> anyhow::Result<Vec<u8>> {
    let headers_list = response.headers().into_iter()
        .map(|(k, v)| -> anyhow::Result<String> {
            Ok(k.to_string() + "\t" + v.to_str()?)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let headers_joined = headers_list.into_iter().reduce(|a, b| a + "\r" + &b);
    let headers_joined = headers_joined.unwrap_or_else(|| "".to_string());
    let header_part = response.status().to_string() + "\n" + /*response.url().as_str() + "\n" +*/ &headers_joined;

    Ok([header_part.as_bytes(), b"\n", &bytes].concat())
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
        let mut iter2 = header_str.splitn(2, |&c| c == b'\r');
        let k = iter2.next().ok_or_else(|| MyCorruptedDBError::default())?;
        let v = iter2.next().ok_or_else(|| MyCorruptedDBError::default())?;
        headers.append(http_for_actix::HeaderName::from_bytes(k)?, http_for_actix::HeaderValue::from_bytes(v)?);
    }

    Ok(response)
}

fn obtain_upstream_url(config: &Data<Config>, req: &actix_web::HttpRequest) -> anyhow::Result<String> {
    let url_prefix = if let Some(upstream_prefix) = &config.upstream_prefix {
        upstream_prefix.clone()
    } else {
        let host = req.headers().get("host")
            .ok_or_else(|| anyhow!("Missing both upstream_prefix in config and Host: header"))?
            .to_str()?;
        "https://".to_string() + host
    };
    Ok(url_prefix + req.path())
}

async fn prepare_request(req: &actix_web::HttpRequest, url: String, body: &web::Bytes, config: &Data<Config>, state: &Data<State>) -> MyResult<reqwest::Request> {

    let request_headers = req.headers().into_iter()
        .map(|h| (h.0.clone(), h.1.clone()))
        .filter(|h|
            !config.request_headers.remove.contains(&h.0.to_string()) ||
                h.0 == http_for_actix::HeaderName::from_static("host"))
        .chain(
            state.as_ref().additional_response_headers.iter().map(|h| (h.0.clone(), h.1.clone()))
        );
    
    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())?;
    let headers = http::HeaderMap::from_iter(
        request_headers
            .map(|h| -> MyResult<_> {
                Ok((
                    http::HeaderName::from_str(h.0.as_str()).map_err(|_| InvalidHeaderNameError::default())?,
                    http::HeaderValue::from_str(h.1.to_str()?).map_err(|_| InvalidHeaderValueError::default())?
                ))
            })
            .into_iter()
            .collect::<MyResult<Vec<_>>>()?
    );
    let builder = state.client.request(method, url).headers(headers).body(Vec::from(body.as_ref()));
    Ok(builder.build()?)
}

async fn proxy(
    req: actix_web::HttpRequest,
    body: web::Bytes,
    config: Data<Config>,
    cache: Data<Arc<tokio::sync::Mutex<Box<BinaryMemCache>>>>,
    state: Data<State>, 
)
    -> MyResult<actix_web::HttpResponse>
{
    // info!("REQ: {:?}", &req);
    info!("Joining proxy received a request to {}", req.path());
    // First level of defence: X-JoinProxy-Key can be stolen by an IC replica owner:
    if let Some(our_secret) = &config.our_secret {
        let passed_key = req.headers()
            .get("x-joinproxy-key")
            .map(|v| v.to_str().map_err(|_| anyhow!("Cannot read header X-JoinProxy-Key")))
            .transpose()?;
        if passed_key != Some(&("Bearer ".to_string() + &our_secret)) {
            return Ok(HttpResponse::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED));
        }
    }

    println!("XXX: {:?}", req.headers()); // FIXME: Remove.     
    let url = obtain_upstream_url(&config, &req)?;
    let serialized_request = serialize_http_request(&req, url.as_str(), &body)?;
    println!("RUST: {:?}", String::from_utf8_lossy(&serialized_request)); // FIXME: Remove.
    let actix_request_hash = Sha256::digest(serialized_request.as_slice());

    let mut cache = (***cache).lock().await;

    // We lock during the time of downloading from upstream to prevent duplicate requests with identical data.
    let mut cache_lock = cache.lock(&Vec::from(actix_request_hash.as_slice())).await?;

    let response = &mut if let Some(serialized_response) = (*cache_lock).inner().await
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
        response
    } else {
        info!("Cache miss.");

        // Second level of defence: Ask back the calling canister.
        // Do it only once per outcall (our response content isn't secure anyway).
        if let (Some(agent), Some(callback)) = (&state.agent, &config.callback) {
            sleep(callback.pause_before_first_call).await;
            // We may do several update calls, but (if so configured) only the last call is paid,
            // thanks to message inspection.
            let start = Instant::now();
            info!("Callback...");
            loop {
                let res = agent.update(&callback.canister, &callback.func)
                    .with_arg(Encode!(&actix_request_hash.as_slice())?).call_and_wait().await; // TODO: call_and_wait()
                match res {
                    Err(e) => {
                        info!("Callback result error: {e}");
                    }
                    Ok(res) => match Decode!(res.as_slice(), ()) { // check for errors
                        Err(e) => {
                            info!("Callback decode error: {e}"); // IC trap
                        }
                        Ok(_) => break,
                    }
                }
                if Instant::now().gt(&start.checked_add(callback.timing_out_calls_after).unwrap()) { // TODO: In principle, this can panic.
                    info!("Callback timeout");
                    return Ok(HttpResponse::GatewayTimeout().body(""));
                }
                sleep(callback.pause_between_calls).await;
            }
            info!("Callback OK.");
        }

        let reqwest = prepare_request(&req, url, &body, &config, &state).await?;
        let reqwest_response = state.client.execute(reqwest).await?;
        info!("Upstream status: {}", reqwest_response.status());

        // We retrieved the response, immediately set and release the cache:
        (*cache_lock).set(Some(serialize_http_response(&reqwest_response, body.clone())?)).await;
        std::mem::drop(cache_lock);

        let mut actix_response = actix_web::HttpResponse::with_body(
            StatusCode::from_u16(reqwest_response.status().as_u16())?, Vec::from(body.as_ref()));

        let headers = actix_response.headers_mut();
        for (k, v) in reqwest_response.headers() {
            headers.append(
                http_for_actix::HeaderName::from_str(k.as_str()).map_err(|_| InvalidHeaderNameError::default())?,
                http_for_actix::HeaderValue::from_str(v.to_str()?).map_err(|_| InvalidHeaderValueError::default())?,
            );
        }

        if config.response_headers.show_hit_miss {
            actix_response.headers_mut().append(
                http_for_actix::HeaderName::from_str("X-JoinProxy-Response").unwrap(),
                http_for_actix::HeaderValue::from_str("Miss").unwrap(),
            );
        }
        // "content-length", "content-encoding" // TODO
        if config.response_headers.add_forwarded_from_header {
            if let Some(addr) = req.head().peer_addr {
                actix_response.headers_mut().append(
                    http_for_actix::HeaderName::from_str("X-Forwarded-For").unwrap(),
                    http_for_actix::HeaderValue::from_str(&addr.ip().to_string()).unwrap(),
                );
            }
        }
        for k in state.response_headers_to_remove.iter() {
            actix_response.headers_mut().remove(k);
        }
        for (k, v) in config.response_headers.add.iter() {
            actix_response.headers_mut().append(
                http_for_actix::HeaderName::from_str(k).map_err(|_| InvalidHeaderNameError::default())?,
                http_for_actix::HeaderValue::from_str(&v).map_err(|_| InvalidHeaderValueError::default())?
            );
        }

        actix_response
    };

    Ok(actix_web::HttpResponse::build(StatusCode::from_u16(response.status().as_u16())?)
        .body(Vec::from(body.as_ref())))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();
    let config_string = read_to_string(&args.config_file)
        .map_err(|e| anyhow!("Cannot read config file {}: {}", args.config_file, e))?;
    let mut config: Config = toml::from_str(&config_string)
        .map_err(|e| anyhow!("Cannot read config file {}: {}", args.config_file, e))?;

    if config.ic_url.is_none() && config.ic_local {
        config.ic_url = Some("http://localhost:8000".to_string())
    }

    // FIXME: "localhost:8081" binds only IPv4, in some reason.
    let server_url = "[::1]".to_string()/*config.serve.host.clone()*/ + ":" + config.serve.port.to_string().as_str();

    let cache = Arc::new(Mutex::new(Box::new(BinaryMemCache::new(config.cache.cache_timeout))));

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
        if config.callback.is_some() {
            let mut builder = Agent::builder();
            if let Some(ic_url) = &config.ic_url {
                builder = builder.with_url(ic_url);
            }
            let agent = builder.build()?;
            if config.ic_local {
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
        let state = State {
            client: ClientBuilder::new()
                .connect_timeout(config.upstream_timeouts.connect_timeout)
                .read_timeout(config.upstream_timeouts.read_timeout)
                .timeout(config.upstream_timeouts.total_timeout)
                .build().unwrap(),
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
                    .with_single_cert(cert_chain, rustls::pki_types::PrivateKeyDer::Pkcs8(key))? // TODO: correct?
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
