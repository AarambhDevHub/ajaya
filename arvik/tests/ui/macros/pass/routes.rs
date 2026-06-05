use arvik::{Path, Router};

#[arvik::get("/users/{id}")]
async fn get_user(Path(id): Path<String>) -> String {
    id
}

#[arvik::post("/users")]
async fn create_user(body: String) -> String {
    body
}

fn main() {
    let _app: Router<()> = Router::new().routes(arvik::collect_routes![get_user, create_user]);
}
