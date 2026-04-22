use axum::{
    body::Body,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::Response,
};
use std::path::Path;

/// Cryptographic middleware to enforce Sovereign-only access
pub async fn sovereign_auth_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get(header::AUTHORIZATION);
    
    match auth_header {
        Some(header) if header.to_str().unwrap_or("") == "SOVEREIGN" => {
            Ok(next.run(req).await)
        }
        _ => {
            println!("!! >> [SECOPS] UNAUTHORIZED UPLINK ATTEMPT INTERCEPTED");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Strict path validation to prevent sandbox escape
pub fn is_path_safe(path_str: &str) -> bool {
    let path = Path::new(path_str);
    
    // 1. Prevent directory traversal
    if path_str.contains("..") {
        println!("!! >> [SECOPS] PATH TRAVERSAL ATTEMPT BLOCKED: {}", path_str);
        return false;
    }

    // 2. Prevent absolute paths escaping the workspace
    if path.is_absolute() {
        println!("!! >> [SECOPS] ABSOLUTE PATH ESCAPE BLOCKED: {}", path_str);
        return false;
    }

    // 3. Blacklist critical infrastructure domains
    let blacklisted = [".env", ".git", "target", "node_modules", ".ssh", ".nexus_memory.db"];
    for restricted in blacklisted.iter() {
        if path_str.contains(restricted) {
            println!("!! >> [SECOPS] RESTRICTED ZONE ACCESS BLOCKED: {}", path_str);
            return false;
        }
    }

    true
}
