// src/diff.rs - Secure diff/patch engine (Indentation Preserved)
use anyhow::{Result, anyhow};

pub struct DiffEngine;

impl DiffEngine {
    pub fn apply(original: &str, search: &str, replace: &str) -> Result<String> {
        if search.is_empty() {
            return Ok(original.to_string());
        }

        // STRICT PROTOCOL: Exact match only, preserving all whitespace/indentation.
        if !original.contains(search) {
            return Err(anyhow!(
                "PATTERN_NOT_FOUND: The exact search string (including indentation) was not located."
            ));
        }

        Ok(original.replace(search, replace))
    }

    pub fn unified_diff(_old: &str, _new: &str, path: &str) -> String {
        format!("--- a/{}\n+++ b/{}\n[MODIFICATION APPLIED]", path, path)
    }
}
