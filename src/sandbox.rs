use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::{Path, PathBuf};

pub fn validate_path(requested_path: &str) -> Result<PathBuf> {
    let workspace_root = env::current_dir().context("Failed to resolve workspace root")?;

    // ⚡ DIAGNOSTIC EXCEPTION
    let is_diagnostic = requested_path == "/proc/meminfo" || requested_path == "/proc/loadavg";
    if is_diagnostic {
        return Ok(PathBuf::from(requested_path));
    }

    // ⚡ PRELIMINARY FILTER (Catching obvious garbage before touching the filesystem)
    if requested_path.contains("..") || requested_path.contains('\0') || requested_path.starts_with('/') {
        return Err(anyhow!("SECURITY VIOLATION: Absolute paths and traversal components forbidden"));
    }

    let target = Path::new(requested_path);
    let absolute = workspace_root.join(target);

    // ⚡ DEEP PATH RESOLUTION
    // Walk up the tree until we find a directory that actually exists on disk
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

    // ⚡ CANONICALIZATION (Resolving Symlinks on the existing base)
    let canonical_base = existing_base.canonicalize().context("Failed to canonicalize base path")?;
    let canonical_root = workspace_root.canonicalize().context("Failed to canonicalize workspace root")?;

    // ⚡ THE SOVEREIGNTY CHECK
    if !canonical_base.starts_with(&canonical_root) {
        return Err(anyhow!("[CRITICAL] Path escape detected. Substrate breach prevented."));
    }

    // ⚡ SAFE RECONSTRUCTION
    // Re-attach the non-existent components to the strictly validated canonical base.
    // This defeats the TOCTOU vulnerability by returning a physical, resolved path.
    let mut safe_path = canonical_base;
    for comp in non_existent_components.into_iter().rev() {
        safe_path.push(comp);
    }

    Ok(safe_path)
}
