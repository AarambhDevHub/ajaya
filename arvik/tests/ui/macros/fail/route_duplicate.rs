use arvik::Router;

#[arvik::get("/dupe")]
async fn first() -> &'static str {
    "first"
}

#[arvik::get("/dupe")]
async fn second() -> &'static str {
    "second"
}

fn main() {
    let _app: Router<()> = Router::new().routes(arvik::collect_routes![first, second]);
}
