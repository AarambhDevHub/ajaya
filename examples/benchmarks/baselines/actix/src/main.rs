use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use serde::Serialize;

#[derive(Serialize)]
struct Message {
    message: &'static str,
}

#[get("/plaintext")]
async fn plaintext() -> impl Responder {
    "Hello, World!"
}

#[get("/json")]
async fn json() -> impl Responder {
    web::Json(Message {
        message: "Hello, World!",
    })
}

#[get("/")]
async fn root() -> impl Responder {
    HttpResponse::Ok().body("Actix baseline")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(root).service(plaintext).service(json))
        .bind(("0.0.0.0", 8082))?
        .run()
        .await
}
