use arvik::debug_handler;

#[debug_handler]
async fn bad(first: String, second: arvik::Body) -> String {
    format!("{first} {:?}", second)
}

fn main() {}
