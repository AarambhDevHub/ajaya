use arvik::{State, debug_handler};

#[derive(Clone)]
struct AppState;

#[debug_handler(state = AppState)]
async fn handler(State(_state): State<AppState>, body: String) -> String {
    body
}

fn main() {
    let _ = handler;
}
