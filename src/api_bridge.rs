use axum::{Router, routing::get_service};
use tower_http::services::ServeDir;
use std::net::SocketAddr;

/// API Bridge for Nexus v2.0
/// Serves the Svelte frontend static assets from ui/dist/
/// Configured for port 3000
pub async fn start_api_bridge() {
    println!(">> [BRIDGE] Initializing Nexus API Bridge...");

    // Define the static file service pointing to the Svelte build directory
    let service = ServeDir::new("./ui/dist");

    // Build the router with a fallback to the static file service
    let app = Router::new()
        .fallback_service(get_service(service));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    
    println!(">> [BRIDGE] Nexus UI Bridge listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port 3000. Ensure no other process is using it.");

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("!! [BRIDGE] API Bridge critical failure: {}", e);
    }
}
