use crate::auth;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub key: String,
}

pub async fn login_handler(Json(payload): Json<LoginRequest>) -> impl IntoResponse {
    let expected = std::env::var("NEXUS_AUTH_KEY").unwrap_or_default();
    if payload.key == expected && !expected.is_empty() {
        match auth::issue_sovereign_token() {
            Ok(token) => Json(serde_json::json!({ "token": token })).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Token gen failed: {}", e)).into_response(),
        }
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid master key").into_response()
    }
}

pub async fn sovereign_auth_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    if req.uri().path() == "/api/auth/login" {
        return Ok(next.run(req).await);
    }

    let auth_header =
        req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = auth_header.trim_start_matches("Bearer ").trim();

    if !token.is_empty() {
        match auth::verify_token_detailed(token) {
            Ok(auth::TokenVerdict::Valid) => return Ok(next.run(req).await),
            Ok(verdict) => tracing::warn!(
                "[SECURITY] 401 Unauthorized: Path={} | Verdict={:?}",
                req.uri().path(),
                verdict
            ),
            Err(e) => tracing::error!(
                "[SECURITY] Auth key misconfiguration at {}: {}",
                req.uri().path(),
                e
            ),
        }
    } else {
        tracing::warn!(
            "[SECURITY] 401 Unauthorized: Path={} | No token provided",
            req.uri().path()
        );
    }
    Err(StatusCode::UNAUTHORIZED)
}

pub fn write_startup_token() -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let token = auth::issue_sovereign_token()?;
    // ⚡ FIX: Strict 0600 permissions
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(".nexus_ui_token")?
        .write_all(token.as_bytes())?;

    println!("\x1b[32m>> [SECURITY] Startup token written to .nexus_ui_token (0600)\x1b[0m");
    Ok(())
}
