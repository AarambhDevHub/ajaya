use arvik::{Json, Router, TestClient, get, post};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct User {
    name: String,
}

async fn home() -> &'static str {
    "Hello from TestClient"
}

async fn create_user(Json(user): Json<User>) -> Json<User> {
    Json(user)
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(home))
        .route("/users", post(create_user));

    let client = TestClient::new(app);

    let res = client.get("/").send().await;
    assert_eq!(res.text().await.unwrap(), "Hello from TestClient");

    let res = client
        .post("/users")
        .json(&User {
            name: "Alice".into(),
        })
        .send()
        .await;
    let user: User = res.json().await.unwrap();
    assert_eq!(user.name, "Alice");

    println!("test client example passed");
}
