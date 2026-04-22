use axum::{
    body::Body,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::Response,
};
use std::path::Path;

pub async fn sovereign_auth_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get(header::AUTHORIZATION);
    
    match auth_header {
        Some(header) if header.to_str().unwrap_or("") == "SOVEREIGN" => {
            Ok(next.run(req).await)
        }
        _ => {
            println!("!! >> [SECOPS] UNAUTHORIZED: {} {}", req.method(), req.uri());
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

pub fn is_path_safe(path_str: &str) -> bool {
    let path = Path::new(path_str);
    if path_str.contains("..") { return false; }
    if path.is_absolute() { return false; }
    let blacklisted = [".env", ".git", "target", "node_modules", ".ssh", ".nexus_memory.db"];
    for restricted in blacklisted.iter() {
        if path_str.contains(restricted) { return false; }
    }
    true
}
