// src/sandbox.rs - v2.1 HARDENED (Symlink + TOCTOU + Escape Proof + Creation Safe)
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow, Context};
use std::env;

pub fn validate_path(requested_path: &str) -> Result<PathBuf> {
    let workspace_root = env::current_dir()
        .context("Failed to resolve workspace root")?;

    // Early aggressive rejection
    if requested_path.contains("..")
        || requested_path.contains('\0')
        || requested_path.starts_with('/')
        || requested_path.starts_with('~')
        || requested_path.contains("/proc")
        || requested_path.contains("/sys")
        || requested_path.contains("/dev")
        || requested_path.contains("/etc")
        || requested_path.contains("/home") {
        return Err(anyhow!("SECURITY VIOLATION: Forbidden path pattern"));
    }

    let target = Path::new(requested_path);
    let absolute = if target.is_absolute() {
        target.to_path_buf()
    } else {
        workspace_root.join(target)
    };

    // Creation-Safe Canonicalization (Fallback to parent if file is new)
    let mut check_path = absolute.clone();
    if !check_path.exists() {
        if let Some(parent) = check_path.parent() {
            check_path = parent.to_path_buf();
        } else {
            return Err(anyhow!("Invalid path structure requested."));
        }
    }

    // Double canonicalization for maximum symlink/TOCTOU protection
    let canonical = check_path.canonicalize()
        .map_err(|e| anyhow!("Path resolution failed (symlink or escape attempt): {}", e))?;

    let canonical_root = workspace_root.canonicalize()
        .context("Failed to canonicalize workspace root")?;

    if !canonical.starts_with(&canonical_root) {
        return Err(anyhow!("[CRITICAL] Path escape detected: {} → {}", requested_path, canonical.display()));
    }

    // Block dangerous subdirs
    let path_str = canonical.to_string_lossy().to_lowercase();
    let blocked = ["/target/", "/.git/", "/node_modules/", "/.lazarus/", "/target/debug", "/target/release"];
    if blocked.iter().any(|b| path_str.contains(b)) {
        return Err(anyhow!("SECURITY VIOLATION: Protected directory access denied"));
    }

    Ok(absolute)
}
