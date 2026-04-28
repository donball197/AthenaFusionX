use axum::{body::Body, http::{Request, StatusCode, header}, middleware::Next, response::Response};
use crate::auth;

pub async fn sovereign_auth_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    if req.uri().path() == "/api/auth/login" {
        return Ok(next.run(req).await);
    }

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth_header.trim_start_matches("Bearer ").trim();

    if !token.is_empty() && auth::verify_token(token).unwrap_or(false) {
        return Ok(next.run(req).await);
    }

    Err(StatusCode::UNAUTHORIZED)
}

pub fn write_startup_token() -> anyhow::Result<()> {
    let token = auth::issue_sovereign_token()?;
    std::fs::write(".nexus_ui_token", &token)?;
    println!("\x1b[32m>> [SECURITY] Startup token written to .nexus_ui_token\x1b[0m");
    Ok(())
}
