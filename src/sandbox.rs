// src/sandbox.rs - v2.4 [Diagnostic Nerve Suture]
use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::{Path, PathBuf};

pub fn validate_path(requested_path: &str) -> Result<PathBuf> {
    let workspace_root = env::current_dir().context("Failed to resolve workspace root")?;

    // SUTURE: Allow read-only access to specific system diagnostics
    let is_diagnostic = requested_path == "/proc/meminfo" || requested_path == "/proc/loadavg";

    if !is_diagnostic && (
        requested_path.contains("..")
        || requested_path.contains('\0')
        || requested_path.starts_with('/')
        || requested_path.starts_with('~')
        || ["/sys", "/dev", "/etc", "/home"].iter().any(|p| requested_path.contains(p))
    ) {
        return Err(anyhow!("SECURITY VIOLATION: Forbidden path pattern"));
    }

    if is_diagnostic {
        return Ok(PathBuf::from(requested_path));
    }

    let target = Path::new(requested_path);
    let absolute = if target.is_absolute() { target.to_path_buf() } else { workspace_root.join(target) };
    let mut check_path = absolute.clone();
    if !check_path.exists() {
        if let Some(parent) = check_path.parent() { check_path = parent.to_path_buf(); }
    }

    let canonical = check_path.canonicalize().map_err(|e| anyhow!("Path resolution failed: {}", e))?;
    let canonical_root = workspace_root.canonicalize().context("Failed to canonicalize workspace root")?;

    if !canonical.starts_with(&canonical_root) {
        return Err(anyhow!("[CRITICAL] Path escape detected"));
    }

    Ok(absolute)
}
