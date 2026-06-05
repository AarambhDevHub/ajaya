#[derive(Clone)]
struct BadHandler;

#[arvik::handler]
impl BadHandler {
    async fn call(&self, _req: String) -> &'static str {
        "bad"
    }
}

fn main() {}
