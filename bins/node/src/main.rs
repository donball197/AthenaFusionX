use axum::{routing::get, Router};
#[tokio::main]
async fn main() {
    println!(">> Edge Node booting...");
    let app = Router::new().route("/", get(|| async { "node alive" }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
