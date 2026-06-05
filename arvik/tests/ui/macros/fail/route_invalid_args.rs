#[arvik::route(BAD, "/bad")]
async fn bad() -> &'static str {
    "bad"
}

fn main() {}
