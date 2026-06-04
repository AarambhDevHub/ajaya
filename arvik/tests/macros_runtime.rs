#![cfg(feature = "macros")]

use arvik::{Body, IntoResponse, Path, Request, Response, Router};
use http::Method;

#[arvik::get("/hello")]
async fn hello() -> &'static str {
    "hello"
}

#[arvik::post("/hello")]
async fn create_hello() -> &'static str {
    "created"
}

#[arvik::get("/users/:id")]
async fn get_user(Path(id): Path<String>) -> String {
    format!("user:{id}")
}

#[derive(Clone)]
struct EchoHandler {
    prefix: &'static str,
}

#[arvik::handler]
impl EchoHandler {
    async fn call(&self, req: Request) -> impl IntoResponse {
        format!("{}{}", self.prefix, req.uri().path())
    }
}

#[tokio::test]
async fn collected_routes_register_and_dispatch() {
    let app: Router<()> =
        Router::new().routes(arvik::collect_routes![hello, create_hello, get_user,]);

    let res = app.call(request(Method::GET, "/hello"), ()).await;
    assert_eq!(res.status(), http::StatusCode::OK);
    assert_eq!(body_text(res).await, "hello");

    let res = app.call(request(Method::POST, "/hello"), ()).await;
    assert_eq!(res.status(), http::StatusCode::OK);
    assert_eq!(body_text(res).await, "created");

    let res = app.call(request(Method::GET, "/users/42"), ()).await;
    assert_eq!(res.status(), http::StatusCode::OK);
    assert_eq!(body_text(res).await, "user:42");
}

#[tokio::test]
async fn handler_impl_block_registers_as_handler() {
    let app: Router<()> =
        Router::new().route("/echo", arvik::get(EchoHandler { prefix: "handled:" }));

    let res = app.call(request(Method::GET, "/echo"), ()).await;
    assert_eq!(res.status(), http::StatusCode::OK);
    assert_eq!(body_text(res).await, "handled:/echo");
}

fn request(method: Method, uri: &str) -> Request {
    Request::new(
        http::Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
    )
}

async fn body_text(response: Response) -> String {
    String::from_utf8(response.into_body().to_bytes().await.unwrap().to_vec()).unwrap()
}
