use anyhow::{anyhow, Result};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const MIN_KEY_BYTES: usize = 32;
const DEFAULT_TTL_SECS: u64 = 86_400;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

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
    std::env::var("NEXUS_TOKEN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TTL_SECS)
}

pub fn issue_sovereign_token() -> Result<String> {
    let key = load_key()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let claims = Claims {
        sub: "architect".to_owned(),
        iat: now as usize,
        exp: (now + token_ttl()) as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(key.as_bytes()),
    )
    .map_err(|e| anyhow!("JWT_GEN_FAIL: {}", e))
}

/// TokenVerdict: Distinguishes between infrastructure failures and policy rejections.
/// Use verify_token_detailed() when you need to log *why* a token was rejected.
#[derive(Debug, PartialEq)]
pub enum TokenVerdict {
    Valid,
    Expired,
    InvalidSignature,
    MissingKey,
}

/// verify_token_detailed: Returns the specific reason for rejection.
/// Err(_) means the auth key is misconfigured — not that the token is bad.
pub fn verify_token_detailed(token: &str) -> Result<TokenVerdict> {
    let key = load_key()?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    match decode::<Claims>(token, &DecodingKey::from_secret(key.as_bytes()), &validation) {
        Ok(_) => Ok(TokenVerdict::Valid),
        Err(e) => Ok(match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenVerdict::Expired,
            _ => TokenVerdict::InvalidSignature,
        }),
    }
}

/// verify_token: Backward-compatible shim — returns true only if Valid.
/// Err(_) indicates a key configuration problem, not a bad token.
pub fn verify_token(token: &str) -> Result<bool> {
    match verify_token_detailed(token)? {
        TokenVerdict::Valid => Ok(true),
        TokenVerdict::Expired => {
            tracing::warn!("[AUTH] Token rejected: expired");
            Ok(false)
        }
        TokenVerdict::InvalidSignature => {
            tracing::warn!("[AUTH] Token rejected: invalid signature");
            Ok(false)
        }
        TokenVerdict::MissingKey => Ok(false), // already an Err from load_key
    }
}
