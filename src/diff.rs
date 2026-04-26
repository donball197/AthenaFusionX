/**
 * Copyright 2026 Donald Ball
 * Licensed under the Apache License, Version 2.0
 */
// src/diff.rs - Secure diff/patch engine — replaces FIRST occurrence only.
use anyhow::{anyhow, Result};

pub struct DiffEngine;

impl DiffEngine {
    /// apply: Replaces the FIRST occurrence of `search` in `original` with `replace`.
    /// Fails if the search string is not found (exact match including whitespace).
    pub fn apply(original: &str, search: &str, replace: &str) -> Result<String> {
        if search.is_empty() {
            return Ok(original.to_string());
        }

        match original.find(search) {
            None => Err(anyhow!(
                "PATTERN_NOT_FOUND: The exact search string (including indentation) was not located."
            )),
            Some(pos) => {
                let mut result = String::with_capacity(
                    original.len() - search.len() + replace.len(),
                );
                result.push_str(&original[..pos]);
                result.push_str(replace);
                result.push_str(&original[pos + search.len()..]);
                Ok(result)
            }
        }
    }

    pub fn unified_diff(_old: &str, _new: &str, path: &str) -> String {
        format!("--- a/{}\n+++ b/{}\n[MODIFICATION APPLIED]", path, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_first_only() {
        let src = "fn foo() {}\nfn foo() {}";
        let result = DiffEngine::apply(src, "fn foo() {}", "fn bar() {}").unwrap();
        assert_eq!(result, "fn bar() {}\nfn foo() {}");
    }

    #[test]
    fn pattern_not_found_errors() {
        assert!(DiffEngine::apply("hello", "world", "earth").is_err());
    }
}
