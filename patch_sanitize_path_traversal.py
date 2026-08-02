#!/usr/bin/env python3
import sys

PATH = "crates/runtime_core/src/executor.rs"

with open(PATH, "r") as f:
    content = f.read()

# Replace any existing sanitize implementation with canonicalized workspace-root check
import re

new_code = '''fn canonicalize_or_parent(p: &Path) -> Result<PathBuf> {
    if let Ok(c) = std::fs::canonicalize(p) {
        return Ok(c);
    }
    let parent = p
        .parent()
        .ok_or_else(|| anyhow!("no parent directory for {}", p.display()))?;
    let parent_canon = std::fs::canonicalize(parent)?;
    let file_name = p
        .file_name()
        .ok_or_else(|| anyhow!("no file name for {}", p.display()))?;
    Ok(parent_canon.join(file_name))
}

pub fn sanitize(path: &str) -> Result<PathBuf> {
    // /tmp/ remains scratch space, outside the workspace-root containment check.
    if path.starts_with("/tmp/") {
        return Ok(Path::new(path).to_path_buf());
    }

    let workspace_root = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .map_err(|e| anyhow!("PATH_TRAVERSAL_DENIED: could not resolve workspace root ({})", e))?;

    let p = Path::new(path);
    let target = if p.is_absolute() { p.to_path_buf() } else { workspace_root.join(p) };

    let canonical = canonicalize_or_parent(&target)
        .map_err(|_| anyhow!("PATH_TRAVERSAL_DENIED: {}", path))?;

    if !canonical.starts_with(&workspace_root) {
        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {} resolves outside the workspace root", path));
    }

    Ok(canonical)
}'''

old_fn_pattern = re.compile(r"pub fn sanitize\(path: &str\) -> Result<PathBuf> \{[\s\S]*?\n\}", re.MULTILINE)
updated_content = old_fn_pattern.sub(new_code, content)

with open(PATH, "w") as f:
    f.write(updated_content)

print(f"[OK] Patched {PATH}")
