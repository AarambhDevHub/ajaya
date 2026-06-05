#[arvik::get("/users/:id")]
async fn bad() -> &'static str {
    "bad"
}

fn main() {}
