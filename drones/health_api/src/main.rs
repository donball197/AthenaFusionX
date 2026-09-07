use axum::{routing::get, Router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(health_check));
    let addr = SocketAddr::from(([0, 0, 0, 0], 7000));
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to initialize health_api bind address");
    println!("Listening on {}", addr);
    axum::serve(listener, app).await.expect("Failed to initialize health_api bind address");
}

async fn health_check() -> &'static str {
    "{\"status\": \"ok\"}"
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = Router::new().route("/health", get(health_check));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("Failed to initialize health_api bind address"),
            )
            .await
            .expect("Failed to initialize health_api bind address");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
