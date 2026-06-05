use arvik::{
    Json, Router, Validate, ValidatedForm, ValidatedJson, ValidatedQuery, get, post, serve_app,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Validate)]
#[validate(crate = "arvik")]
struct CreateUser {
    #[validate(length(min = 2, max = 50))]
    name: String,
    #[validate(email)]
    email: String,
    #[validate(range(min = 18, max = 120))]
    age: u8,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(crate = "arvik")]
struct LoginForm {
    #[validate(length(min = 2))]
    username: String,
    #[validate(length(min = 8))]
    password: String,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(crate = "arvik")]
struct SearchQuery {
    #[validate(length(min = 2))]
    q: String,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    name: String,
    email: String,
    age: u8,
}

async fn create_user(ValidatedJson(user): ValidatedJson<CreateUser>) -> Json<UserResponse> {
    Json(UserResponse {
        name: user.name,
        email: user.email,
        age: user.age,
    })
}

async fn login(ValidatedForm(form): ValidatedForm<LoginForm>) -> String {
    format!("welcome {}", form.username)
}

async fn search(ValidatedQuery(query): ValidatedQuery<SearchQuery>) -> String {
    format!("searching for {}", query.q)
}

async fn home() -> &'static str {
    "request validation example"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .route("/", get(home))
        .route("/users", post(create_user))
        .route("/login", post(login))
        .route("/search", get(search));

    serve_app("0.0.0.0:8080", app).await?;
    Ok(())
}
