use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::{Path, PathBuf};

const DIAGNOSTIC_ALLOWLIST: &[&str] = &[
    "/proc/loadavg",
    "/proc/meminfo",
    "/sys/class/thermal/thermal_zone0/temp",
];

pub fn validate_path(requested_path: &str) -> Result<PathBuf> {
    if requested_path.contains('\0') {
        return Err(anyhow!("SECURITY VIOLATION: Null byte in path"));
    }

    if DIAGNOSTIC_ALLOWLIST.binary_search(&requested_path).is_ok() {
        return Ok(PathBuf::from(requested_path));
    }

    let workspace_root = env::current_dir().context("Failed to resolve workspace root")?;

    if requested_path.contains("..") || requested_path.starts_with('/') {
        return Err(anyhow!("SECURITY VIOLATION: Absolute paths and traversal components forbidden"));
    }

    let target   = Path::new(requested_path);
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

    let canonical_base = existing_base.canonicalize().context("Failed to canonicalize base path")?;
    let canonical_root = workspace_root.canonicalize().context("Failed to canonicalize workspace root")?;

    if !canonical_base.starts_with(&canonical_root) {
        return Err(anyhow!("[CRITICAL] Path escape detected. Substrate breach prevented."));
    }

    let mut safe_path = canonical_base;
    for comp in non_existent_components.into_iter().rev() {
        safe_path.push(comp);
    }
    Ok(safe_path)
}

pub fn normalize_and_check(path: &str) -> Result<PathBuf> {
    if path.contains('\0') {
        return Err(anyhow!("SECURITY VIOLATION: Null byte in path"));
    }
    if path.contains("..") || path.starts_with('/') {
        return Err(anyhow!("SECURITY VIOLATION: Path must be relative and within workspace"));
    }
    let workspace = env::current_dir().context("Failed to resolve workspace root")?;
    let candidate = workspace.join(path);
    if candidate.exists() {
        let canonical = candidate.canonicalize().context("Failed to canonicalize")?;
        let canonical_root = workspace.canonicalize().context("Failed to canonicalize root")?;
        if !canonical.starts_with(&canonical_root) {
            return Err(anyhow!("[CRITICAL] Path escape detected."));
        }
        Ok(canonical)
    } else {
        Ok(candidate)
    }
}
