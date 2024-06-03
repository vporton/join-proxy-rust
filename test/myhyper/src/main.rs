extern crate hyper;
extern crate hyper_rustls;
extern crate hyper_tls;
extern crate http;
// extern crate hyper_rustls;

use std::time::Duration;

use hyper::{client::connect::HttpConnector, Body, Client};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
// use hyper_tls::HttpsConnector;
use http::{Method, Uri};

#[tokio::main]
async fn main() {
    let uri = Uri::from_static("https://local.vporton.name:8443/");

    let mut http_connector = HttpConnector::new();
    http_connector.enforce_http(false);
    http_connector
        .set_connect_timeout(Some(Duration::from_secs(2)));

    // Https client setup.
    let https_connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .https_only()
        .enable_http1()
        .wrap_connector(http_connector);
    let https_client: Client<HttpsConnector<HttpConnector>> = Client::builder().build::<_, hyper::Body>(https_connector);

    let mut http_req = hyper::Request::new(Body::from(""));
    // *http_req.headers_mut() = headers;
    *http_req.method_mut() = Method::GET;
    *http_req.uri_mut() = uri.clone();
    https_client.request(http_req).await.map_err(|e| format!("Failed to directly connect: {e}")).expect("Can't download");
}