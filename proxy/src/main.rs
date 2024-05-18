mod errors;
mod cache;

use std::{fs::File, str::{from_utf8, FromStr}, sync::{Arc, Mutex}, time::Duration};

use actix::clock::sleep;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use lib::{get_canister_pubkey, verify_signature, CanisterPublicKeyPollResult, CanisterPublicKeyStatus};
use log::info;
use serde::Deserializer;
use sha2::Digest;
use actix_web::{body::BoxBody, http::StatusCode, web::{self, Data}, App, HttpResponse, HttpServer};
use anyhow::{anyhow, bail};
use cache::cache::{Cache, Key, Value};
use clap::Parser;
use errors::{InvalidHeaderNameError, InvalidHeaderValueError, MyCorruptedDBError, MyResult};
use reqwest::ClientBuilder;
use serde_derive::Deserialize;
use sha2::Sha256;
use k256::ecdsa::{Signature, VerifyingKey};
use ic_agent::{export::Principal, Agent};
use serde::de::Error;

use crate::cache::mem_cache::MemCache;

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long="config", default_value="config.json")]
    config_file: String,
}

#[derive(Clone, Deserialize, Debug)]
struct Config {
    #[serde(default="default_port")]
    port: u16,
    our_secret: Option<String>, // simple Bearer authentication
    upstream_prefix: Option<String>,
    cache_timeout: Duration,
    remove_request_headers: Vec<String>,
    add_request_headers: Vec<(String, String)>,
    remove_response_headers: Vec<String>,
    add_response_headers: Vec<(String, String)>,
    #[serde(default="default_show_hit_miss")]
    show_hit_miss: bool,
    #[serde(default="default_upstream_connect_timeout")]
    upstream_connect_timeout: Duration,
    #[serde(default="default_upstream_read_timeout")]
    upstream_read_timeout: Duration,
    #[serde(default="default_add_forwarded_from_header")]
    add_forwarded_from_header: bool,
    ic_url: Option<String>,
    #[serde(default="default_ic_local")]
    ic_local: bool,
    #[serde(deserialize_with = "deserialize_canister_id")]
    signing_canister_id: Option<Principal>,
}

fn default_port() -> u16 {
    8080
}

fn default_show_hit_miss() -> bool {
    true
}

fn default_upstream_connect_timeout() -> Duration {
    Duration::from_secs(10)
}

fn default_upstream_read_timeout() -> Duration {
    Duration::from_secs(60) // I set it big, for the use case of OpenAI API
}

fn default_add_forwarded_from_header() -> bool {
    false // Isn't it useless?
}

fn default_ic_local() -> bool {
    false
}

fn deserialize_canister_id<'de, D>(deserializer: D) -> Result<Option<Principal>, D::Error>
where
    D: Deserializer<'de>,
{
    let input: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    if let Some(input) = input {
        match Principal::from_text(input) {
            Ok(principal) => Ok(Some(principal)),
            Err(principal_error) =>
                Err(D::Error::custom(format!("Invalid principal: {}", principal_error))),
        }
    } else {
        Ok(None)
    }
}

struct State {
    client: reqwest::Client,
    verifying_key: Option<VerifyingKey>,
    additional_response_headers: Arc<Vec<(http_for_actix::HeaderName, http_for_actix::HeaderValue)>>,
    response_headers_to_remove: Arc<Vec<http_for_actix::HeaderName>>,
}

// Two similar functions with different data types follow:

fn serialize_http_request(request: &actix_web::HttpRequest, bytes: &actix_web::web::Bytes) -> anyhow::Result<Vec<u8>> {
    let headers_list = request.headers().into_iter()
        .map(|(k, v)| -> anyhow::Result<String> {
            Ok(k.to_string() + "\t" + v.to_str()?)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let headers_joined = headers_list.into_iter().reduce(|a, b| a + "\r" + &b);
    let headers_joined = headers_joined.unwrap_or_else(|| "".to_string());
    let header_part = request.method().as_str().to_owned() + "\n" + &request.uri().to_string() + "\n" + &headers_joined;

    Ok([header_part.as_bytes(), b"\n", bytes.to_vec().as_slice()].concat())
}

fn serialize_http_response(response: reqwest::Response, bytes: bytes::Bytes) -> anyhow::Result<Vec<u8>> {
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

async fn prepare_request(req: &actix_web::HttpRequest, body: &web::Bytes, config: &Data<Config>, state: &Data<State>) -> MyResult<reqwest::Request> {
    let url_prefix = if let Some(upstream_prefix) = &config.upstream_prefix {
        upstream_prefix.clone()
    } else {
        let host = req.headers().get("x-host") // TODO: Document X-Host.
            .ok_or_else(|| anyhow!("Missing both upstream_prefix in config and Host: header"))?
            .to_str()?;
        "https://".to_string() + host
    };
    let url = url_prefix + req.path();

    let request_headers = req.headers().into_iter()
        .map(|h| (h.0.clone(), h.1.clone()))
        .filter(|h|
            !config.remove_request_headers.contains(&h.0.to_string()) ||
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

// TODO: Use `&[u8]` instead of `BoxBody`.
async fn serve(
    req: actix_web::HttpRequest,
    body: web::Bytes,
    config: Data<Config>,
    cache: Data<Arc<Mutex<&mut dyn Cache>>>,
    state: Data<State>,
)
    -> MyResult<actix_web::HttpResponse<BoxBody>>
{
    let serialized_request = serialize_http_request(&req, &body)?;

    let mut cache = cache.lock().unwrap();
    let response = &mut if let Some(serialize_response) =
        cache.get(Key(serialized_request.as_slice()))?
    {
        let mut response = deserialize_http_response(serialize_response)?;
        if config.show_hit_miss {
            response.headers_mut().append(
                http_for_actix::HeaderName::from_str("X-JoinProxy-Response").unwrap(),
                http_for_actix::HeaderValue::from_str("Hit").unwrap(),
            );
        }
        response
    } else {
        let reqwest = prepare_request(&req, &body, &config, &state).await?;
        let reqwest_response = state.client.execute(reqwest).await?;

        let mut actix_response = actix_web::HttpResponse::with_body(
            StatusCode::from_u16(reqwest_response.status().as_u16())?, Vec::from(body.as_ref()));

        let headers = actix_response.headers_mut();
        for (k, v) in reqwest_response.headers() {
            headers.append(
                http_for_actix::HeaderName::from_str(k.as_str()).map_err(|_| InvalidHeaderNameError::default())?,
                http_for_actix::HeaderValue::from_str(v.to_str()?).map_err(|_| InvalidHeaderValueError::default())?,
            );
        }

        // TODO: After which headers modifications to put this block?
        let hash = Sha256::digest(serialized_request.as_slice());
        cache.put(Key(&hash), Value(serialize_http_response(reqwest_response, body.clone())?.as_slice()))?;

        if config.show_hit_miss {
            actix_response.headers_mut().append(
                http_for_actix::HeaderName::from_str("X-JoinProxy-Response").unwrap(),
                http_for_actix::HeaderValue::from_str("Miss").unwrap(),
            );
        }
        // "content-length", "content-encoding" // TODO
        if config.add_forwarded_from_header {
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
        for (k, v) in config.add_response_headers.iter() {
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

async fn proxy(
    req: actix_web::HttpRequest,
    body: web::Bytes,
    config: Data<Config>,
    cache: Data<Arc<Mutex<&mut dyn Cache>>>,
    state: Data<State>, 
)
    -> MyResult<actix_web::HttpResponse>
{
    if let Some(our_secret) = &config.our_secret {
        let passed_key = req.headers()
            .get("x-joinproxy-key")
            .map(|v| v.to_str().map_err(|_| anyhow!("Cannot read header X-JoinProxy-Key")))
            .transpose()?;
        if passed_key != Some(&("Bearer ".to_string() + &our_secret)) {
            return Ok(HttpResponse::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED));
        }
    }
    if let Some(verifying_key) = &state.verifying_key {
        if let (Some(nonce_header), Some(signature_as_base64)) =
            (req.headers().get("nonce"), req.headers().get("signature"))
        {
            let mut nonce_iter = nonce_header.as_bytes().split(|&c| c == b':');
            // let long_time_nonce_as_base64 = nonce_iter.next().ok_or_else(|| anyhow!("Wrong nonce."))?;
            let short_time_nonce_as_base64 = nonce_iter.next().ok_or_else(|| anyhow!("Wrong nonce."))?;
            if nonce_iter.next().is_some() {
                return Err(anyhow!("Wrong nonce").into());
            }
            let signature = STANDARD_NO_PAD.decode(signature_as_base64)?;
            // let long_time_nonce = STANDARD_NO_PAD.decode(long_time_nonce_as_base64)?;
            let short_time_nonce = STANDARD_NO_PAD.decode(short_time_nonce_as_base64)?;
            // FIXME: Verify no repeated short_time_nonce.
            let hash = Sha256::digest(nonce_header.as_bytes());
            if verify_signature(Signature::from_bytes(
                signature.as_slice().into())?, &hash.into() as &[u8; 32], *verifying_key
            ).is_err() {
                return Ok(HttpResponse::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED));
            }
        } else {
            return Ok(HttpResponse::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED));
        }
    }

    serve(req, body, config, cache, state).await
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let file = File::open(&args.config_file)
        .map_err(|e| anyhow!("Cannot open config file {}: {}", args.config_file, e))?;
    let config: Config = serde_json::from_reader(file)
        .map_err(|e| anyhow!("Cannot read config file {}: {}", args.config_file, e))?;

    let server_url = "localhost:".to_string() + config.port.to_string().as_str();

    let cache = Arc::new(Mutex::new(MemCache::new(config.cache_timeout)));

    let additional_response_headers = &config.add_request_headers;
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
            .chain(config.remove_response_headers.iter().map(|s| s.as_str()))
            .map(|h| http_for_actix::HeaderName::from_str(h).map_err(|_| InvalidHeaderNameError::default().into()));
    let response_headers_to_remove = response_headers_to_remove.collect::<MyResult<Vec<_>>>()?;
    let response_headers_to_remove = Arc::new(response_headers_to_remove);

    // TODO: Download verifying key in parallel with serving.

    let agent = {
        let mut builder = Agent::builder();
        if let Some(ic_url) = &config.ic_url {
            builder = builder.with_url(ic_url);
        }
        builder.build()?
    };
    if config.ic_local {
        agent.fetch_root_key().await?;
    }

    let verifying_key =
        if let Some(signing_canister_id) = config.signing_canister_id {
            'b: {
                let verifying_key_status = get_canister_pubkey(&agent, signing_canister_id).await?;
                for i in 0..20 { // TODO: Make configurable.
                    info!("Downloading the key, attempt {}", i+1);
                    match CanisterPublicKeyStatus::poll(&agent, &verifying_key_status).await {
                        Ok(CanisterPublicKeyPollResult::Submitted) => {}
                        Ok(CanisterPublicKeyPollResult::Completed(key)) => {
                            break 'b Some(key);
                        }
                        Ok(CanisterPublicKeyPollResult::Accepted) => bail!("Canister key disappeared."),
                        Err(err) => bail!("Failed to load canister key: {err}"),
                    }
                    sleep(Duration::from_millis(500)).await; // TODO: Make configurable.
                }

                bail!("Cannot load canister public key!");
            }
        } else {
            None
        };

    HttpServer::new(move || {
        let state = State {
            client: ClientBuilder::new()
                .connect_timeout(config.upstream_connect_timeout)
                .read_timeout(config.upstream_read_timeout)
                .build().unwrap(),
            additional_response_headers: additional_response_headers.clone(),
            response_headers_to_remove: response_headers_to_remove.clone(),
            verifying_key,
        };
        App::new().service(
            web::scope("")
            .app_data(Data::new(config.clone()))
            .app_data(Data::new(state))
            .app_data(Data::new(cache.clone()))
                .route("/{_:.*}", web::route().to(proxy))
        )
    })
        .bind(server_url)?
        .run()
        .await.map_err(|e| e.into())
}
