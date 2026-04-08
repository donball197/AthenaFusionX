use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use std::env;

/// Validates that a requested path strictly resides within the Nexus workspace.
/// Neutralizes directory traversal attacks (e.g., "../../../etc/shadow").
pub fn validate_path(requested_path: &str) -> Result<PathBuf> {
    let workspace_root = env::current_dir()?;
    let target_path = Path::new(requested_path);
    
    // Resolve to an absolute path first
    let absolute_target = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        workspace_root.join(target_path)
    };

    // To prevent symlink escapes and normalize ".." traversals, we must canonicalize.
    // Canonicalization requires the path to exist. If creating a new file, we check the parent.
    let mut check_path = absolute_target.clone();
    if !check_path.exists() {
        if let Some(parent) = check_path.parent() {
            check_path = parent.to_path_buf();
        } else {
            return Err(anyhow!("Invalid path structure requested."));
        }
    }

    // Resolve the actual physical paths on the ARM64 disk
    let canonical_target = check_path.canonicalize()
        .map_err(|e| anyhow!("Sandbox resolution failed: {}", e))?;
    let canonical_root = workspace_root.canonicalize()
        .map_err(|e| anyhow!("Workspace root resolution failed: {}", e))?;

    // The core security check: Does the target physically reside inside the root?
    if canonical_target.starts_with(&canonical_root) {
        Ok(absolute_target)
    } else {
        Err(anyhow!(
            "[SECURITY VIOLATION] Agent attempted directory traversal outside workspace bounds: {}", 
            requested_path
        ))
    }
}
