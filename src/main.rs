use std::str::from_utf8;

use actix_web::http::{header::{HeaderName, HeaderValue}, StatusCode};
use anyhow::anyhow;

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
        let mut iter2 = header_str.splitn(2, |&c| c == b'\r'); // TODO: error, if longer than 2
        let k = iter2.next().ok_or_else(|| anyhow!("Wrong data in DB."))?;
        let v = iter2.next().ok_or_else(|| anyhow!("Wrong data in DB."))?;
        headers.append(HeaderName::from_bytes(k)?, HeaderValue::from_bytes(v)?);
    }

    Ok(response)
}

#[actix::main]
async fn main() {
    println!("Hello, world!");
}
