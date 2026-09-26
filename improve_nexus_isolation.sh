#!/usr/bin/env bash
# improve_nexus_isolation.sh
# Applies the workspace-isolation fix + related robustness improvements
# for nexus_ast_tool dry-run / self-optimize pipeline.
set -euo pipefail

SUPERVISOR="apps/nexus_ast_tool/src/supervisor.rs"
BACKUP="${SUPERVISOR}.bak.$(date +%s)"

if [[ ! -f "$SUPERVISOR" ]]; then
  echo "ERROR: $SUPERVISOR not found. Run from the AthenaFusionx repo root."
  exit 1
fi

echo "==> Backing up $SUPERVISOR → $BACKUP"
cp "$SUPERVISOR" "$BACKUP"

echo "==> Applying improvements..."

python3 - <<'PY'
from pathlib import Path
import re

path = Path("apps/nexus_ast_tool/src/supervisor.rs")
src = path.read_text()

# ------------------------------------------------------------------
# 1. Replace the naive find_nearest_manifest + simple parent() logic
#    with proper workspace-root discovery.
# ------------------------------------------------------------------

old_find = '''
fn find_nearest_manifest(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        let candidate = current.join("Cargo.toml");

        if candidate.is_file() {
            return Some(candidate);
        }

        if !current.pop() {
            return None;
        }
    }
}
'''

new_helpers = '''
fn find_nearest_manifest(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        let candidate = current.join("Cargo.toml");

        if candidate.is_file() {
            return Some(candidate);
        }

        if !current.pop() {
            return None;
        }
    }
}

/// Walk upward from a package manifest until we locate the real Cargo
/// workspace root (the directory whose Cargo.toml contains a `[workspace]`
/// table). Falls back to the package directory when no workspace is found.
///
/// This is required for isolation: member crates that inherit dependencies
/// via `workspace = true` cannot be built without the workspace root.
fn find_workspace_root(manifest_path: &Path) -> PathBuf {
    let package_dir = manifest_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut current = package_dir.clone();

    loop {
        let candidate = current.join("Cargo.toml");
        if candidate.is_file() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                // Sufficient for the V1 isolation contract.
                // A full TOML parse would also work but adds a dependency
                // and is unnecessary for this gate.
                if content.contains("[workspace]") {
                    return current;
                }
            }
        }
        if !current.pop() {
            // No workspace found — treat the original package dir as root.
            return package_dir;
        }
    }
}
'''

if old_find.strip() not in src:
    # Try a more tolerant match in case of whitespace differences
    old_find_re = re.compile(
        r'fn find_nearest_manifest\(start: &Path\) -> Option<PathBuf> \{.*?\n\}',
        re.DOTALL,
    )
    if not old_find_re.search(src):
        print("WARNING: could not locate find_nearest_manifest – skipping helper injection")
    else:
        src = old_find_re.sub(new_helpers.strip(), src, count=1)
        print("  [✓] injected find_workspace_root helper")
else:
    src = src.replace(old_find.strip(), new_helpers.strip())
    print("  [✓] injected find_workspace_root helper")

# ------------------------------------------------------------------
# 2. Update BuildProfile::discover to use the real workspace root
# ------------------------------------------------------------------

old_discover = '''
    pub fn discover<P: AsRef<Path>>(target: P) -> anyhow::Result<Self> {
        let target = target.as_ref();

        let start = if target.is_dir() {
            target
        } else {
            target
                .parent()
                .ok_or_else(|| anyhow!("Target has no parent directory"))?
        };

        let manifest_path = find_nearest_manifest(start)
            .ok_or_else(|| anyhow!("No Cargo.toml found for target {}", target.display()))?;

        let workspace_root = manifest_path
            .parent()
            .ok_or_else(|| anyhow!("Cargo manifest has no parent directory"))?
            .to_path_buf();

        Ok(Self {
            workspace_root,
            manifest_path,
        })
    }
'''

new_discover = '''
    pub fn discover<P: AsRef<Path>>(target: P) -> anyhow::Result<Self> {
        let target = target.as_ref();

        let start = if target.is_dir() {
            target
        } else {
            target
                .parent()
                .ok_or_else(|| anyhow!("Target has no parent directory"))?
        };

        let manifest_path = find_nearest_manifest(start)
            .ok_or_else(|| anyhow!("No Cargo.toml found for target {}", target.display()))?;

        // CRITICAL: resolve the real workspace root, not just the member package.
        // Member crates that use `workspace = true` cannot be built in isolation.
        let workspace_root = find_workspace_root(&manifest_path);

        Ok(Self {
            workspace_root,
            manifest_path,
        })
    }
'''

if old_discover.strip() in src:
    src = src.replace(old_discover.strip(), new_discover.strip())
    print("  [✓] updated BuildProfile::discover to use real workspace root")
else:
    # Fallback: more tolerant regex
    discover_re = re.compile(
        r'pub fn discover<P: AsRef<Path>>\(target: P\) -> anyhow::Result<Self> \{.*?\n    \}',
        re.DOTALL,
    )
    if discover_re.search(src):
        src = discover_re.sub(new_discover.strip(), src, count=1)
        print("  [✓] updated BuildProfile::discover (regex)")
    else:
        print("WARNING: could not locate BuildProfile::discover – please apply manually")

# ------------------------------------------------------------------
# 3. Small robustness: make copy_tree skip more build artifacts
# ------------------------------------------------------------------

old_skip = '''
        if name == "target" || name == ".git" {
            continue;
        }
'''

new_skip = '''
        // Skip heavy / non-source directories that would bloat the isolated
        // workspace and are irrelevant for the dry-run contract.
        if name == "target"
            || name == ".git"
            || name == "node_modules"
            || name == ".venv"
            || name == "venv"
            || name == "__pycache__"
            || name == "dist"
            || name == "build"
        {
            continue;
        }
'''

if old_skip.strip() in src:
    src = src.replace(old_skip.strip(), new_skip.strip())
    print("  [✓] expanded copy_tree exclusion list")
else:
    print("  [·] copy_tree exclusion already updated or not found")

path.write_text(src)
print("==> Wrote updated", path)
PY

echo ""
echo "==> Verifying the change compiles (cargo check -p nexus_ast_tool)..."
if cargo check -p nexus_ast_tool --quiet 2>/dev/null; then
  echo "  [✓] cargo check passed"
else
  echo "  [!] cargo check reported errors – inspect the diff and fix manually"
  echo "      Backup is at: $BACKUP"
fi

echo ""
echo "Done. Next steps:"
echo "  1. Review the diff:  git diff $SUPERVISOR"
echo "  2. Re-run a dry-run against a workspace member (e.g. crates/runtime_core)."
echo "  3. The isolated workspace will now contain the real monorepo root,"
echo "     so 'cargo check' will no longer fail with 'failed to find a workspace root'."
