use anyhow::{anyhow, Result};
use axum::{http::StatusCode, response::IntoResponse, Json};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

const MIN_KEY_BYTES: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

pub async fn login_handler(Json(payload): Json<LoginRequest>) -> impl IntoResponse {
    let secret = std::env::var("NEXUS_AUTH_KEY").unwrap_or_default();
    if payload.password == secret {
        match issue_sovereign_token() {
            Ok(token) => {
                (StatusCode::OK, Json(serde_json::json!({ "token": token }))).into_response()
            }
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "TOKEN_GEN_FAIL" })),
            )
                .into_response(),
        }
    } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "INVALID_CREDENTIALS" })))
            .into_response()
    }
}

pub fn load_key_at_startup() -> anyhow::Result<Zeroizing<String>> {
    let key = std::env::var("NEXUS_AUTH_KEY")
        .map_err(|_| anyhow::anyhow!("NEXUS_AUTH_KEY env var is not set — refusing to start"))?;
    if key.len() < MIN_KEY_BYTES {
        return Err(anyhow::anyhow!(
            "NEXUS_AUTH_KEY is too short ({} bytes). Minimum is {} bytes.",
            key.len(),
            MIN_KEY_BYTES
        ));
    }
    Ok(Zeroizing::new(key))
}

const DEFAULT_TTL_SECS: u64 = 86_400;

fn load_key() -> Result<String> {
    let key = std::env::var("NEXUS_AUTH_KEY")
        .map_err(|_| anyhow!("NEXUS_AUTH_KEY env var is not set"))?;
    if key.len() < MIN_KEY_BYTES {
        return Err(anyhow!(
            "NEXUS_AUTH_KEY is too short ({} bytes). Minimum is {} bytes.",
            key.len(),
            MIN_KEY_BYTES
        ));
    }
    Ok(key)
}

fn token_ttl() -> u64 {
    match std::env::var("NEXUS_TOKEN_TTL_SECS") {
        Ok(v) => match v.parse::<u64>() {
            Ok(secs) => secs,
            Err(_) => DEFAULT_TTL_SECS,
        },
        Err(_) => DEFAULT_TTL_SECS,
    }
}

pub fn issue_sovereign_token() -> Result<String> {
    let key = load_key()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let claims = Claims {
        sub: "architect".to_owned(),
        iat: now as usize,
        exp: (now + token_ttl()) as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(key.as_bytes()))
        .map_err(|e| anyhow!("JWT_GEN_FAIL: {}", e))
}

#[derive(Debug, PartialEq)]
pub enum TokenVerdict {
    Valid,
    Expired,
    InvalidSignature,
}

pub fn verify_token_detailed(token: &str) -> Result<TokenVerdict> {
    let key = load_key()?;
    let validation = Validation::new(Algorithm::HS256);
    match decode::<Claims>(token, &DecodingKey::from_secret(key.as_bytes()), &validation) {
        Ok(_) => Ok(TokenVerdict::Valid),
        Err(e) => match *e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => Ok(TokenVerdict::Expired),
            _ => Ok(TokenVerdict::InvalidSignature),
        },
    }
}
