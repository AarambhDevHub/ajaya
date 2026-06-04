use arvik::{State, debug_handler};

#[derive(Clone)]
struct AppState;

#[debug_handler]
async fn bad(State(_state): State<AppState>) -> &'static str {
    "bad"
}

fn main() {}
