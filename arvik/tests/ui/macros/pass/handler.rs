use arvik::{IntoResponse, Request};

#[derive(Clone)]
struct ServiceHandler;

#[arvik::handler]
impl ServiceHandler {
    async fn call(&self, req: Request) -> impl IntoResponse {
        req.uri().path().to_string()
    }
}

fn main() {
    let _router: arvik::MethodRouter<()> = arvik::get(ServiceHandler);
}
