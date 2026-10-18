//! Shared mutation revert. Tracked files are restored via `git checkout`;
//! untracked files (created by the agent) are deleted. Paths that escape
//! the repo root are refused.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("not inside a git work tree".to_string());
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    PathBuf::from(s).canonicalize().map_err(|e| e.to_string())
}

/// Ok(true) = tracked, Ok(false) = git says unknown path, Err = git failed.
fn is_tracked(rel: &str) -> Result<bool, String> {
    let out = Command::new("git")
        .args(["ls-files", "--error-unmatch", "--", rel])
        .output()
        .map_err(|e| e.to_string())?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        other => Err(format!("git ls-files failed: {:?}", other)),
    }
}

/// Returns Ok(()) only when `path` is back to its pre-mutation state.
pub(crate) fn revert_mutation_path(path: &str) -> Result<(), String> {
    let root = repo_root()?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let joined = cwd.join(path);

    let file_name =
        joined.file_name().ok_or_else(|| "path has no file name".to_string())?.to_os_string();
    let parent = match joined.parent() {
        Some(p) => p,
        None => return Err("path has no parent".to_string()),
    };
    // Parent missing means the file cannot exist: nothing to revert.
    let parent = match parent.canonicalize() {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };
    if !parent.starts_with(&root) {
        return Err(format!("path escapes repo root: {}", path));
    }
    let target = parent.join(file_name);
    let rel = target.strip_prefix(&root).map_err(|e| e.to_string())?.to_string_lossy().to_string();

    if is_tracked(&rel)? {
        let ok = crate::podman::PodmanShield::execute_argv("git", &["checkout", "--", &rel])
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            Ok(())
        } else {
            Err(format!("git checkout failed for {}", rel))
        }
    } else if target.exists() {
        std::fs::remove_file(&target).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
