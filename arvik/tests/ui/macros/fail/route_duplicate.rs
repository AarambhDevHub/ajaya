use arvik::Router;

#[arvik::get("/dupe")]
async fn first() -> &'static str {
    "first"
}

fn main() {
    let _app: Router<()> = Router::new().routes(arvik::collect_routes![first, first]);
}
