//! REST API helpers for the hold review system.
//!
//! Provides Bearer auth and serialization types matching hold.rs.
//! HTTP routing is handled by the caller (axum/actix in the backend).

use serde::{Deserialize, Serialize};

/// Bearer token guard: returns true if the Authorization header matches.
pub fn check_bearer(auth_header: &str, expected_token: &str) -> bool {
    auth_header
        .strip_prefix("Bearer ")
        .map(|t| t.trim() == expected_token)
        .unwrap_or(false)
}

/// Summary of a hold for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldSummary {
    pub id: String,
    pub files_held: usize,
    pub root: String,
    pub created_unix: u64,
}

/// Full detail of a hold for diff views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldDetail {
    pub id: String,
    pub created_unix: u64,
    pub authoritative_root: String,
    pub single_file: bool,
    pub files: Vec<HeldFileDetail>,
}

/// Per-file detail within a hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeldFileDetail {
    pub rel: String,
    pub status: String,
    pub findings: Vec<HeldFindingDetail>,
    pub orig_sha256: String,
    pub patched_sha256: String,
}

/// Per-finding detail within a held file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeldFindingDetail {
    pub pattern: String,
    pub line: usize,
    pub patch: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_valid() {
        assert!(check_bearer("Bearer secret123", "secret123"));
    }

    #[test]
    fn bearer_invalid() {
        assert!(!check_bearer("Bearer wrong", "secret123"));
        assert!(!check_bearer("Basic abc", "secret123"));
        assert!(!check_bearer("", "secret123"));
        assert!(!check_bearer("secret123", "secret123"));
    }

    #[test]
    fn summary_serialization() {
        let s = HoldSummary {
            id: "test".into(),
            files_held: 1,
            root: "/tmp".into(),
            created_unix: 12345,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("test"));
    }
}
