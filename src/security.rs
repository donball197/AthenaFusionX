use axum::{body::Body, http::{Request, StatusCode, header}, middleware::Next, response::Response};
use crate::auth;

pub async fn sovereign_auth_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // ⚡ Allow the login endpoint to be accessed without a token
    if req.uri().path() == "/api/auth/login" { return Ok(next.run(req).await); }

    let provided = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");

    if auth::verify_token(provided).unwrap_or(false) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
