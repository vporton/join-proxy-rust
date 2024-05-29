use actix_web::{web, App, HttpResponse, HttpServer};
use log::info;

async fn test_page() -> HttpResponse {
    info!("Test server received a request.");
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Test")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    HttpServer::new(|| {
        App::new().route("/", web::get().to(test_page))
    })
    .bind(("localhost", 8081))?
    .run()
    .await
}
