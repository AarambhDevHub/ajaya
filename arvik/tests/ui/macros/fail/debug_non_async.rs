use arvik::debug_handler;

#[debug_handler]
fn bad() -> &'static str {
    "bad"
}

fn main() {}
