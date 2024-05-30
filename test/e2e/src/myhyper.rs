use hyper::{client::Client, Uri};
use hyper_tls::HttpsConnector;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use log::{info, error};
use env_logger;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the logger from the environment
    env_logger::init();

    // Read the URL from the command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: https_downloader <URL> <output_file>");
        std::process::exit(1);
    }
    let url = &args[1];
    let output_file = &args[2];

    // Parse the URL
    let uri = url.parse::<Uri>()
        .map_err(|e| format!("Failed to parse URL: {}", e))?;

    // Create the HTTPS connector
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    // Make the request
    match client.get(uri).await {
        Ok(response) => {
            info!("Response: {}", response.status());
            let body = hyper::body::to_bytes(response.into_body()).await?;
            let mut file = File::create(output_file).await?;
            file.write_all(&body).await?;
            info!("Downloaded {} bytes to {}", body.len(), output_file);
        }
        Err(e) => {
            error!("Error sending request: {:?}", e);
        }
    }

    Ok(())
}
