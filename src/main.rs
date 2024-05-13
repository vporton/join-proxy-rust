mod errors;

use std::{collections::HashMap, fs::File, str::{from_utf8, FromStr}};

use actix_web::{http::StatusCode, web::{self, Data}, App, HttpResponse, HttpServer};
use anyhow::anyhow;
use clap::Parser;
use errors::MyResult;
use reqwest::Client;
use serde_derive::Deserialize;

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
    our_secret: Option<String>, // simple Bearer authentication // TODO: Don't authenticate with bearer, if uses another auth.
    upstream_prefix: Option<String>,
    remove_request_headers: Vec<String>,
    add_request_headers: HashMap<String, String>,
    remove_response_headers: Vec<String>,
    add_response_headers: HashMap<String, String>,
}

fn default_port() -> u16 {
    8080
}

// Two similar functions with different data types follow:

fn serialize_http_request(request: actix_web::HttpRequest, bytes: actix_web::web::Bytes) -> anyhow::Result<Vec<u8>> {
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

fn deserialize_http_response(data: &[u8]) -> anyhow::Result<actix_web::HttpResponse<&[u8]>> {
    let mut iter1 = data.splitn(3, |&c| c == b'\n');
    // TODO: Eliminate duplicate error messages.
    let status_code_bytes = iter1.next().ok_or_else(|| anyhow!("Wrong data in DB."))?;
    let headers_bytes = iter1.next().ok_or_else(|| anyhow!("Wrong data in DB."))?;
    let body = iter1.next().ok_or_else(|| anyhow!("Wrong data in DB."))?;

    let status_code: u16 = str::parse(from_utf8(status_code_bytes)?)?;
    let mut response = actix_web::HttpResponse::with_body(
        StatusCode::from_u16(status_code)?, body);

    let headers = response.headers_mut();
    for header_str in headers_bytes.split(|&c| c == b'\r') {
        let mut iter2 = header_str.splitn(2, |&c| c == b'\r');
        let k = iter2.next().ok_or_else(|| anyhow!("Wrong data in DB."))?;
        let v = iter2.next().ok_or_else(|| anyhow!("Wrong data in DB."))?;
        headers.append(http_for_actix::HeaderName::from_bytes(k)?, http_for_actix::HeaderValue::from_bytes(v)?);
    }

    Ok(response)
}

async fn prepare_request(req: actix_web::HttpRequest, body: web::Bytes, config: Data<Config>) -> MyResult<reqwest::Request> {
    let url_prefix = if let Some(upstream_prefix) = &config.upstream_prefix {
        upstream_prefix.clone()
    } else {
        let host = req.headers().get("host")
            .ok_or_else(|| anyhow!("Missing both upstream_prefix in config and Host: header"))?
            .to_str()?;
        "https://".to_string() + host
    };
    let url = url_prefix + req.path();

    // TODO: Calculate `additional_headers` only once.
    let additional_headers = &config.add_request_headers;
    let additional_headers = additional_headers.into_iter().map(
        |v| -> MyResult<_> {
            Ok((
                http_for_actix::HeaderName::from_str(&v.0).map_err(|_| anyhow!("Invalid header name."))?,
                http_for_actix::HeaderValue::from_str(&v.1).map_err(|_| anyhow!("Invalid header value."))?
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let request_headers = req.headers().into_iter()
        .map(|h| (h.0.clone(), h.1.clone()))
        .filter(|h|
            !config.remove_request_headers.contains(&h.0.to_string()) ||
                h.0 == http_for_actix::HeaderName::from_static("host"))
        .chain(
            additional_headers.into_iter().map(|h| (h.0, h.1))
        );
    
    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())?;
    // TODO: .timeout()
    let client = Client::new(); // TODO: Cache. // TODO: No duplicate variable
    let headers = http::HeaderMap::from_iter(
        request_headers
            .map(|h| -> MyResult<_> {
                Ok((
                    http::HeaderName::from_str(h.0.as_str()).map_err(|_| anyhow!("Invalid header name."))?,
                    http::HeaderValue::from_str(h.1.to_str()?).map_err(|_| anyhow!("Invalid header value."))?
                ))
            })
            .into_iter()
            .collect::<MyResult<Vec<_>>>()?
    );
    let builder = client.request(method, url).headers(headers).body(body);
    Ok(builder.build()?)
}

async fn serve(req: actix_web::HttpRequest, body: web::Bytes, config: Data<Config>) -> MyResult<actix_web::HttpResponse> {
    let serialized_request = serialize_http_request(req, body)?;

    let response = if let Some(serialize_response) = cache.get(serialized_request.as_slice()) {
        let response = deserialize_http_response(serialize_response)?;
        response.headers_mut().append(
            http_for_actix::HeaderName::from_str("X-JoinProxy-Response").unwrap(),
            http_for_actix::HeaderValue::from_str("Hit").unwrap(),
        );
        response
    } else {
        let client = Client::new(); // TODO: Cache. // TODO: No duplicate variable
        let reqwest = prepare_request(req, body, config).await?;
        let response = client.execute(reqwest).await?;

        let mut response = actix_web::HttpResponse::with_body(
            StatusCode::from_u16(response.status().as_u16())?, body.as_ref());

        // cache.put() // TODO

        let headers = response.headers_mut();
        for (k, v) in response.headers() {
            headers.append(
                http_for_actix::HeaderName::from_str(k.as_str()).map_err(|_| anyhow!("Invalid header name."))?,
                http_for_actix::HeaderValue::from_str(v.to_str()?).map_err(|_| anyhow!("Invalid header value."))?
            );
        }
        response.headers_mut().append(
            http_for_actix::HeaderName::from_str("X-JoinProxy-Response").unwrap(),
            http_for_actix::HeaderValue::from_str("Miss").unwrap(),
        );

        //  http://tools.ietf.org/html/rfc2616#section-13.5.1
        let hop_by_hop = ["connection", "keep-alive", "te", "trailers", "transfer-encoding", "upgrade"];
        let headers_to_remove = // TODO: Calculate once.
            hop_by_hop.into_iter().chain(config.remove_response_headers.into_iter().map(|s| s.as_str()));
        // "content-length", "content-encoding" // TODO
        if let Some(addr) = req.head().peer_addr { // TODO
            response.headers_mut().append(
                http_for_actix::HeaderName::from_str("X-Forwarded-For").unwrap(),
                http_for_actix::HeaderValue::from_str(&addr.ip().to_string()).unwrap(),
            );
        }
        for k in headers_to_remove {
            response.headers_mut().remove(k);
        }
        for (k, v) in config.add_response_headers.iter() {
            response.headers_mut().append(
                http_for_actix::HeaderName::from_str(k).map_err(|_| anyhow!("Invalid header name."))?,
                http_for_actix::HeaderValue::from_str(&v).map_err(|_| anyhow!("Invalid header value."))?
            );
        }

        response
    };

    Ok(actix_web::HttpResponse::build(StatusCode::from_u16(response.status().as_u16())?)
        .body(*response.body())) // TODO: streaming
}

async fn proxy(req: actix_web::HttpRequest, body: web::Bytes, config: Data<Config>) -> MyResult<actix_web::HttpResponse> {
    if let Some(our_secret) = &config.our_secret {
        let passed_key = req.headers()
            .get("x-joinproxy-key")
            .map(|v| v.to_str().map_err(|_| anyhow!("Cannot read header X-JoinProxy-Key")))
            .transpose()?;
        if passed_key != Some(&("Bearer ".to_string() + &our_secret)) {
            return Ok(HttpResponse::new(StatusCode::NETWORK_AUTHENTICATION_REQUIRED));
        }
    }

    serve(req, body, config).await
}

#[actix_web::main]
async fn main() -> MyResult<()> {
    let args = Args::parse();
    let file = File::open(&args.config_file)
        .map_err(|e| anyhow!("Cannot open config file {}: {}", args.config_file, e))?;
    let config: Config = serde_json::from_reader(file)
        .map_err(|e| anyhow!("Cannot read config file {}: {}", args.config_file, e))?;

    let server_url = "localhost:".to_string() + config.port.to_string().as_str();
    HttpServer::new(move || App::new().service(
        web::scope("")
            .app_data(Data::new(config.clone()))
            .route("/{_:.*}", web::route().to(proxy)))
    )
        .bind(server_url)?
        .run()
        .await.map_err(|e| e.into())
}
