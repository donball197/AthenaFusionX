/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/diff.rs - Secure diff/patch engine (Indentation Preserved)
use anyhow::{anyhow, Result};

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
