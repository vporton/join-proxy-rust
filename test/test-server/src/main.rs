use actix_web::{web, App, HttpResponse, HttpServer};

async fn test_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Test")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().route("/", web::get().to(test_page))
    })
    .bind(("localhost", 8081))?
    .run()
    .await
}
