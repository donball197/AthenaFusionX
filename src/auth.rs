use anyhow::{anyhow, Result};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn issue_sovereign_token() -> Result<String> {
    let key = std::env::var("NEXUS_AUTH_KEY").map_err(|_| anyhow!("NEXUS_AUTH_KEY missing"))?;
    let expiration = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 86400;

    let claims = Claims { sub: "architect".to_owned(), exp: expiration as usize };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(key.as_bytes()))
        .map_err(|e| anyhow!("JWT_GEN_FAIL: {}", e))
}

pub fn verify_token(token: &str) -> Result<bool> {
    let key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_default();
    let validation = Validation::new(Algorithm::HS256);
    match decode::<Claims>(token, &DecodingKey::from_secret(key.as_bytes()), &validation) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
