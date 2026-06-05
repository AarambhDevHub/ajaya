#[derive(Clone)]
struct BadHandler;

#[arvik::handler]
impl BadHandler {
    async fn other(&self, _req: arvik::Request) -> &'static str {
        "bad"
    }
}

fn main() {}
