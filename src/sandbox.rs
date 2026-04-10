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


// src/sandbox.rs - v2.3 HARDENED (Zero-Allocation Blocklist)
use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::{Component, Path, PathBuf};

pub fn validate_path(requested_path: &str) -> Result<PathBuf> {
    let workspace_root = env::current_dir().context("Failed to resolve workspace root")?;

    if requested_path.contains("..")
        || requested_path.contains('\0')
        || requested_path.starts_with('/')
        || requested_path.starts_with('~')
        || requested_path.contains("/proc")
        || requested_path.contains("/sys")
        || requested_path.contains("/dev")
        || requested_path.contains("/etc")
        || requested_path.contains("/home")
    {
        return Err(anyhow!("SECURITY VIOLATION: Forbidden path pattern"));
    }

    let target = Path::new(requested_path);
    let absolute =
        if target.is_absolute() { target.to_path_buf() } else { workspace_root.join(target) };

    let mut check_path = absolute.clone();
    let is_new_file = !check_path.exists();
    if is_new_file {
        if let Some(parent) = check_path.parent() {
            check_path = parent.to_path_buf();
        } else {
            return Err(anyhow!("Invalid path structure requested."));
        }
    }

    let canonical = check_path
        .canonicalize()
        .map_err(|e| anyhow!("Path resolution failed (symlink or escape attempt): {}", e))?;

    let canonical_root =
        workspace_root.canonicalize().context("Failed to canonicalize workspace root")?;

    if !canonical.starts_with(&canonical_root) {
        return Err(anyhow!("[CRITICAL] Path escape detected"));
    }

    // SUTURE: Zero-allocation component inspection.
    for component in canonical.components() {
        if let Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                if matches!(name_str, "target" | ".git" | "node_modules" | ".lazarus") {
                    return Err(anyhow!("SECURITY VIOLATION: Protected directory access denied"));
                }
            }
        }
    }

    if is_new_file {
        let filename = absolute.file_name().ok_or_else(|| anyhow!("Invalid filename"))?;
        Ok(canonical.join(filename))
    } else {
        Ok(canonical)
    }
}
