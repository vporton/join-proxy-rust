mod errors;

use std::{fs::File, str::from_utf8};

use actix_web::{http::{header::{HeaderName, HeaderValue}, StatusCode}, web::{self, Data}, App, HttpResponse, HttpServer};
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

// TODO: Use lifetimes to return `&[u8]` rather than `Vec`.
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
        headers.append(HeaderName::from_bytes(k)?, HeaderValue::from_bytes(v)?);
    }

    Ok(response)
}

// FIXME
async fn serve(req: actix_web::HttpRequest, body: web::Bytes) -> MyResult<actix_web::HttpResponse> {
    let target_url = ""; // FIXME

    // let mut builder = client
    //     .request_from(req.path(), req.head())
    //     .no_decompress();
    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())?;
    // TODO: .timeout()
    let client = Client::new(); // TODO: Cache.
    let reqwest = client.request(method, target_url)/*.headers(headers)*/.body(body).build()?;
    let response = client.execute(reqwest).await?;

    if let Some(addr) = req.head().peer_addr {
        // builder = builder.header("X-Forwarded-For", addr.ip().to_string());
    }

    // let res = builder.send_body(body.into()).await?;

    Ok(actix_web::HttpResponse::build(StatusCode::from_u16(response.status().as_u16())?)
        .append_header(("X-Proxied", "true"))
        .body(response.bytes().await?)) // TODO: streaming
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

    serve(req, body).await
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
