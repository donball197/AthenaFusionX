// sandbox.rs — v2.1
// FIX: Diagnostic exception now uses an allowlist constant instead of
//      an inline string-equality check that could be accidentally expanded.
// FIX: Added explicit check for null bytes before path operations.

use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::{Path, PathBuf};

/// Paths that are allowed outside the workspace for diagnostic reads only.
const DIAGNOSTIC_ALLOWLIST: &[&str] = &[
    "/proc/meminfo",
    "/proc/loadavg",
    "/sys/class/thermal/thermal_zone0/temp",
];

pub fn validate_path(requested_path: &str) -> Result<PathBuf> {
    // Reject null bytes before any OS call
    if requested_path.contains('\0') {
        return Err(anyhow!("SECURITY VIOLATION: Null byte in path"));
    }

    // Diagnostic allowlist — explicit set, not open-ended
    if DIAGNOSTIC_ALLOWLIST.contains(&requested_path) {
        return Ok(PathBuf::from(requested_path));
    }

    let workspace_root = env::current_dir().context("Failed to resolve workspace root")?;

    if requested_path.contains("..")
        || requested_path.starts_with('/')
    {
        return Err(anyhow!(
            "SECURITY VIOLATION: Absolute paths and traversal components forbidden"
        ));
    }

    let target = Path::new(requested_path);
    let absolute = workspace_root.join(target);

    let mut existing_base = absolute.clone();
    let mut non_existent_components = Vec::new();

    while !existing_base.exists() {
        if let Some(comp) = existing_base.file_name() {
            non_existent_components.push(comp.to_owned());
        }
        if !existing_base.pop() {
            return Err(anyhow!("Failed to resolve path ancestry"));
        }
    }

    let canonical_base =
        existing_base.canonicalize().context("Failed to canonicalize base path")?;
    let canonical_root =
        workspace_root.canonicalize().context("Failed to canonicalize workspace root")?;

    if !canonical_base.starts_with(&canonical_root) {
        return Err(anyhow!("[CRITICAL] Path escape detected. Substrate breach prevented."));
    }

    let mut safe_path = canonical_base;
    for comp in non_existent_components.into_iter().rev() {
        safe_path.push(comp);
    }

    Ok(safe_path)
}
