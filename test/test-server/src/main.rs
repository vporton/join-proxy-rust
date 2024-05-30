use std::{fs::File, io::BufReader};

use derive_more::From;
use thiserror::Error;
use anyhow::Context;
use rustls::ServerConfig;
use rustls_pemfile::{certs, pkcs8_private_keys};
use actix_web::{web, App, HttpResponse, HttpServer};
use log::info;
use anyhow::anyhow;

async fn test_page() -> HttpResponse {
    info!("Test server received a request.");
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Test")
}

#[derive(Debug, Error, From)]
pub enum MyError {
    #[error("{0}")]
    Anyhow(Box<anyhow::Error>),
    #[error("{0}")]
    Io(std::io::Error),
}

impl From<anyhow::Error> for MyError {
    fn from(err: anyhow::Error) -> Self {
        Self::Anyhow(Box::new(err))
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cert_file = &mut BufReader::new(File::open("localhost.pem").context("Can't read HTTPS cert.")?);
    let key_file = &mut BufReader::new(File::open("localhost.key").context("Can't read HTTPS key.")?);
    let cert_chain = certs(cert_file).collect::<Result<Vec<_>, _>>()
        .context("Can't parse HTTPS certs chain.")?;
    let key = pkcs8_private_keys(key_file)
        .next().transpose()?.ok_or(anyhow!("No private key in the file."))?;

    HttpServer::new(|| {
        App::new().route("/", web::get().to(test_page))
    })
        .bind_rustls_0_23(
            "localhost:8081",
            ServerConfig::builder().with_no_client_auth()
                .with_single_cert(cert_chain, rustls::pki_types::PrivateKeyDer::Pkcs8(key))?
        )?
        .run()
        .await.map_err(|e| e.into())
}
