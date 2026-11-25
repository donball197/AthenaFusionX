//! REST API server for the hold review system.
//!
//! Exposes GET/POST /api/holds with Bearer token authentication.
//! This module was lost during a branch merge — the API surface is
//! reconstructed from the hold.rs integration points and the Svelte
//! dashboard's uplink.js client.

use crate::hold::HoldStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldSummary {
    pub id: String,
    pub files_held: usize,
    pub root: String,
    pub created_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldDetail {
    pub id: String,
    pub created_unix: u64,
    pub authoritative_root: String,
    pub single_file: bool,
    pub files: Vec<HeldFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeldFile {
    pub rel: String,
    pub status: String,
    pub findings: Vec<HeldFinding>,
    pub orig_sha256: String,
    pub patched_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeldFinding {
    pub pattern: String,
    pub line: usize,
    pub patch: String,
}

/// Bearer token guard: returns true if the Authorization header matches.
pub fn check_bearer(auth_header: &str, expected_token: &str) -> bool {
    auth_header
        .strip_prefix("Bearer ")
        .map(|t| t.trim() == expected_token)
        .unwrap_or(false)
}

/// List all holds as summaries (for the dashboard overview).
pub fn list_holds(store: &HoldStore) -> Vec<HoldSummary> {
    store
        .list()
        .into_iter()
        .map(|h| HoldSummary {
            id: h.id.clone(),
            files_held: h.files.len(),
            root: h.authoritative_root.clone(),
            created_unix: h.created_unix,
        })
        .collect()
}

/// Get a single hold's full detail (for the diff view).
pub fn get_hold(store: &HoldStore, id: &str) -> Option<HoldDetail> {
    store.get(id).map(|h| HoldDetail {
        id: h.id.clone(),
        created_unix: h.created_unix,
        authoritative_root: h.authoritative_root.clone(),
        single_file: h.single_file,
        files: h
            .files
            .iter()
            .map(|f| HeldFile {
                rel: f.rel.clone(),
                status: format!("{:?}", f.status),
                findings: f
                    .findings
                    .iter()
                    .map(|fi| HeldFinding {
                        pattern: fi.pattern.clone(),
                        line: fi.line,
                        patch: fi.patch.clone(),
                    })
                    .collect(),
                orig_sha256: f.orig_sha256.clone(),
                patched_sha256: f.patched_sha256.clone(),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_auth_valid() {
        assert!(check_bearer("Bearer mysecret123", "mysecret123"));
    }

    #[test]
    fn test_bearer_auth_invalid() {
        assert!(!check_bearer("Bearer wrong", "mysecret123"));
        assert!(!check_bearer("Basic dXNlcg==", "mysecret123"));
        assert!(!check_bearer("", "mysecret123"));
    }

    #[test]
    fn test_bearer_auth_missing_prefix() {
        assert!(!check_bearer("mysecret123", "mysecret123"));
    }
}
