use arvik::{Path, debug_handler};

#[debug_handler]
async fn bad(body: String, Path(_id): Path<String>) -> String {
    body
}

fn main() {}
