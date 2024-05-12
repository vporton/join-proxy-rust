use actix_web::http::header::{HeaderName, HeaderValue};

fn serialize_http_request(request: actix_web::HttpRequest, bytes: actix_web::web::Bytes) -> anyhow::Result<Vec<u8>> {
    let headers_list = request.headers().into_iter()
        .map(|(k, v): (&HeaderName, &HeaderValue)| -> anyhow::Result<String> {
            Ok(k.to_string() + "\t" + v.to_str()?)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let headers_joined = headers_list.into_iter().reduce(|a, b| a + "\r" + &b);
    let headers_joined = headers_joined.unwrap_or_else(|| "".to_string());
    let header_part = request.method().as_str().to_owned() + "\n" + &request.uri().to_string() + "\n" + &headers_joined + "\n";

    Ok([header_part.as_bytes(), bytes.to_vec().as_slice()].concat())
}

#[actix::main]
async fn main() {
    println!("Hello, world!");
}
