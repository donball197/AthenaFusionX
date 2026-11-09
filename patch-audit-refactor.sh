#!/usr/bin/env bash
# patch-audit-refactor.sh
# Fixes two findings from the workspace audit:
#   Finding 7: output-file write pattern duplicated in 4 apps, with a
#              shared error-swallowing bug (let _ = create_dir_all).
#   Finding 6: parse_arg duplicated in 5 apps (4 identical + 1 diverged).
# Creates crates/nexus_cli_utils, migrates the four write sites
# automatically, and migrates parse_arg only where the body is verified
# to match symbol_graph's canonical implementation.
set -euo pipefail

usage() {
    cat <<'USAGE_EOF'
Usage: patch-audit-refactor.sh [--apply] [--check] [--skip-parse-arg] [REPO_ROOT]

  (no flags)         Dry-run: print unified diff of all changes.
  --apply            Write changes to working tree.
  --check            After applying, run cargo check --workspace --all-targets.
  --skip-parse-arg   Skip Finding 6 (parse_arg) even if verification passes.
  REPO_ROOT          Defaults to current directory.
USAGE_EOF
    exit 1
}

APPLY=0; CHECK=0; SKIP_PARSE=0; REPO_ROOT=""
for arg in "$@"; do
    case "$arg" in
        --apply) APPLY=1 ;;
        --check) CHECK=1 ;;
        --skip-parse-arg) SKIP_PARSE=1 ;;
        -h|--help) usage ;;
        -*) echo "Unknown flag: $arg" >&2; usage ;;
        *) REPO_ROOT="$arg" ;;
    esac
done
REPO_ROOT="${REPO_ROOT:-$PWD}"
cd "$REPO_ROOT"

if [ ! -f Cargo.toml ] || [ ! -d apps ] || [ ! -d crates ]; then
    echo "error: not at workspace root ($REPO_ROOT)" >&2; exit 1
fi

python3 - "$APPLY" "$SKIP_PARSE" <<'PY'
import sys, re, difflib
from pathlib import Path
from collections import OrderedDict

APPLY      = sys.argv[1] == "1"
SKIP_PARSE = sys.argv[2] == "1"

statuses = []   # (kind, message)
files    = OrderedDict()   # path -> [orig, final]

def load(p):
    if p not in files:
        files[p] = [Path(p).read_text()] * 2
    return files[p]

def edit(p, desc, old, new, kind="will apply"):
    if not Path(p).exists():
        statuses.append(("MISSING FILE", f"{p}: {desc}")); return False
    orig, final = load(p)
    if new in final and old not in final:
        statuses.append(("already applied", desc)); return True
    if old not in final:
        statuses.append(("PREIMAGE NOT FOUND", f"{p}: {desc}")); return False
    files[p][1] = final.replace(old, new, 1)
    statuses.append((kind, desc)); return True

# ─────────────────────────────────────────────────────────────────────
# 1. Create crates/nexus_cli_utils
# ─────────────────────────────────────────────────────────────────────
UTIL_DIR = Path("crates/nexus_cli_utils")
UTIL_CARGO = UTIL_DIR / "Cargo.toml"
UTIL_LIB   = UTIL_DIR / "src/lib.rs"

UTIL_CARGO_CONTENT = '''[package]
name = "nexus_cli_utils"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"

[dev-dependencies]
tempfile = "3"
'''

UTIL_LIB_CONTENT = r'''//! Shared CLI helpers for `apps/*` binaries.
//!
//! Consolidated from the 2026-11-09 workspace audit:
//!   - `parse_arg` was duplicated verbatim across 5 apps
//!     (symbol_graph, mutation_ranker, execution_auditor,
//!     benchmark_runner, impact_analyzer). The last one had
//!     diverged to `flag: &str` and is migrated separately.
//!   - The "create parent directory, then write" pattern was
//!     duplicated across 4 apps with a shared bug: `let _ =
//!     fs::create_dir_all(parent)` discarded the creation error,
//!     so a permission/ENOSPC failure surfaced later as the
//!     misleading "No such file or directory" from `fs::write`.
//!     `write_text_output` propagates the directory-creation
//!     error with its own context message.

use anyhow::Context;
use std::path::Path;

/// Scan `args` for any of `flags`, returning the following argument.
///
/// Matches the first flag found. Does NOT support `--flag=value` syntax
/// (the pre-consolidation copies did not either; preserving behavior).
/// Returns `None` if no flag matches, or if a flag is the last arg.
pub fn parse_arg(args: &[String], flags: &[&str]) -> Option<String> {
    for flag in flags {
        if let Some(i) = args.iter().position(|r| r == flag) {
            if let Some(val) = args.get(i + 1) {
                if !val.starts_with('-') {
                    return Some(val.clone());
                }
            }
        }
    }
    None
}

/// Ensure the parent directory of `path` exists.
///
/// Handles the bare-filename case (`Path::new("out.json")`), whose
/// `.parent()` returns `Some("")` — `create_dir_all("")` is a std
/// no-op, but the explicit guard documents the intent.
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Write `content` to `path`, ensuring the parent directory exists first.
///
/// Directory-creation failures are attributed to directory creation,
/// not to the subsequent write (which would otherwise see ENOENT and
/// report a confusing message).
pub fn write_text_output(path: &Path, content: &str) -> anyhow::Result<()> {
    ensure_parent_dir(path)
        .with_context(|| format!("Failed to create directory for {:?}", path))?;
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write output to {:?}", path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_arg_finds_flag() {
        let args = vec!["--in".into(), "f.rs".into()];
        assert_eq!(parse_arg(&args, &["--in"]), Some("f.rs".into()));
    }

    #[test]
    fn parse_arg_accepts_any_of_flags() {
        let args = vec!["-i".into(), "f.rs".into()];
        assert_eq!(parse_arg(&args, &["--in", "-i"]), Some("f.rs".into()));
    }

    #[test]
    fn parse_arg_none_when_absent() {
        let args = vec!["--other".into(), "x".into()];
        assert_eq!(parse_arg(&args, &["--in"]), None);
    }

    #[test]
    fn parse_arg_none_when_flag_is_last() {
        let args = vec!["--in".into()];
        assert_eq!(parse_arg(&args, &["--in"]), None);
    }

    #[test]
    fn parse_arg_skips_trailing_flag() {
        let args = vec!["--in".into(), "--output".into(), "f.json".into()];
        assert_eq!(parse_arg(&args, &["--in"]), None);
    }

    #[test]
    fn parse_arg_finds_value_after_sibling_flag() {
        let args = vec!["--in".into(), "--output".into(), "f.json".into()];
        assert_eq!(parse_arg(&args, &["--output"]), Some("f.json".into()));
    }

    #[test]
    fn parse_arg_priority_follows_flag_order() {
        let args = vec!["-i".into(), "a".into(), "--in".into(), "b".into()];
        assert_eq!(parse_arg(&args, &["--in", "-i"]), Some("b".into()));
        assert_eq!(parse_arg(&args, &["-i", "--in"]), Some("a".into()));
    }

    #[test]
    fn write_text_output_creates_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a/b/c/out.json");
        write_text_output(&path, "{}").unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    }

    #[test]
    fn write_text_output_bare_filename() {
        // Bare filename: parent() is Some(""), must not error.
        let tmp = tempfile::tempdir().unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = write_text_output(Path::new("out.json"), "{}");
        std::env::set_current_dir(cwd).unwrap();
        result.unwrap();
        assert!(tmp.path().join("out.json").exists());
    }
}
'''

if UTIL_CARGO.exists():
    statuses.append(("exists", f"{UTIL_CARGO}"))
else:
    files[UTIL_CARGO] = [None, UTIL_CARGO_CONTENT]
    statuses.append(("will create", str(UTIL_CARGO)))

if UTIL_LIB.exists():
    statuses.append(("exists", f"{UTIL_LIB}"))
else:
    files[UTIL_LIB] = [None, UTIL_LIB_CONTENT]
    statuses.append(("will create", str(UTIL_LIB)))

# ─────────────────────────────────────────────────────────────────────
# 2. Workspace member registration
# ─────────────────────────────────────────────────────────────────────
root_cargo = Path("Cargo.toml").read_text()
if re.search(r'["\']crates/\*["\']', root_cargo) or re.search(r'["\']crates/[^"\'*]+["\']', root_cargo):
    statuses.append(("info", "workspace members: crates/* glob or explicit list covers new crate"))
else:
    statuses.append(("WARNING", "root Cargo.toml does not list crates/* — add crates/nexus_cli_utils manually"))

# ─────────────────────────────────────────────────────────────────────
# 3. Finding 7 — migrate the four output-write sites
# ─────────────────────────────────────────────────────────────────────
F7_APPS = [
    "apps/mutation_ranker/src/main.rs",
    "apps/execution_auditor/src/main.rs",
    "apps/benchmark_runner/src/main.rs",
    "apps/impact_analyzer/src/main.rs",
]
F7_OLD = (
    "        if let Some(parent) = out_path.parent() {\n"
    "            let _ = fs::create_dir_all(parent);\n"
    "        }\n"
    "        fs::write(&out_path, &json_output)\n"
    "            .with_context(|| format!(\"Failed to write output to {:?}\", out_path))?;\n"
)
F7_NEW = "        nexus_cli_utils::write_text_output(&out_path, &json_output)?;\n"

for path in F7_APPS:
    if not Path(path).exists():
        statuses.append(("MISSING FILE", path)); continue
    edit(path, f"Finding 7: replace output-write block in {path}", F7_OLD, F7_NEW)

    # Update the app's Cargo.toml to depend on nexus_cli_utils
    app_dir  = Path(path).parent.parent
    app_cargo = app_dir / "Cargo.toml"
    if not app_cargo.exists():
        statuses.append(("MISSING FILE", str(app_cargo))); continue
    cargo_txt = app_cargo.read_text()
    if "nexus_cli_utils" in cargo_txt:
        statuses.append(("already applied", f"dep in {app_cargo}"))
        continue
    dep_line = 'nexus_cli_utils = { path = "../../crates/nexus_cli_utils" }\n'
    # Insert under [dependencies]; if the section is missing, append
    if "[dependencies]" in cargo_txt:
        m = re.search(r'\[dependencies\]\n', cargo_txt)
        insert_at = m.end()
        files[str(app_cargo)] = [
            cargo_txt,
            cargo_txt[:insert_at] + dep_line + cargo_txt[insert_at:],
        ]
        statuses.append(("will apply", f"add dep in {app_cargo}"))
    else:
        files[str(app_cargo)] = [
            cargo_txt,
            cargo_txt.rstrip() + "\n\n[dependencies]\n" + dep_line,
        ]
        statuses.append(("will apply", f"add [dependencies] + dep in {app_cargo}"))

# ─────────────────────────────────────────────────────────────────────
# 4. Finding 6 — migrate parse_arg where body matches canonical
# ─────────────────────────────────────────────────────────────────────
CANON_PATH = "apps/symbol_graph/src/main.rs"

def extract_fn_block(src, name):
    """Return (start_idx, end_idx, full_block_including_fn) or None."""
    m = re.search(r'(?m)^fn\s+' + re.escape(name) + r'\s*\(', src)
    if not m: return None
    brace = src.find('{', m.end())
    if brace == -1: return None
    depth = 0; i = brace
    while i < len(src):
        if src[i] == '{': depth += 1
        elif src[i] == '}':
            depth -= 1
            if depth == 0:
                end = i + 1
                while end < len(src) and src[end] in '\n':
                    end += 1
                return (m.start(), end, src[m.start():end])
        i += 1
    return None

def normalize_body(block):
    """Strip the fn signature and outer braces; collapse whitespace."""
    m = re.match(r'fn\s+\w+\s*\(.*?\)\s*->\s*[^{]+\{', block, re.DOTALL)
    if not m:
        m = re.match(r'fn\s+\w+\s*\(.*?\)\s*\{', block, re.DOTALL)
    if not m: return block
    inner = block[m.end():block.rfind('}')]
    return re.sub(r'\s+', ' ', inner).strip()

def insert_use_line(src, module_path):
    """Insert `use <module_path>;` after the last top-level `use` statement."""
    matches = list(re.finditer(r'(?m)^use\s+[^\n]+;\n', src))
    if not matches:
        # No top-level use; insert after leading attributes/mod decls. Fallback: at top.
        return f"use {module_path};\n" + src
    last = matches[-1]
    return src[:last.end()] + f"use {module_path};\n" + src[last.end():]

if SKIP_PARSE:
    statuses.append(("info", "Finding 6 skipped (--skip-parse-arg)"))
elif not Path(CANON_PATH).exists():
    statuses.append(("WARNING", f"canonical parse_arg source missing: {CANON_PATH}"))
else:
    canon_src = Path(CANON_PATH).read_text()
    canon_block = extract_fn_block(canon_src, "parse_arg")
    if not canon_block:
        statuses.append(("WARNING", f"could not extract fn parse_arg from {CANON_PATH}"))
    else:
        canon_norm = normalize_body(canon_block[2])
        F6_APPS = [
            "apps/symbol_graph/src/main.rs",
            "apps/mutation_ranker/src/main.rs",
            "apps/execution_auditor/src/main.rs",
            "apps/benchmark_runner/src/main.rs",
            "apps/impact_analyzer/src/main.rs",
        ]
        for path in F6_APPS:
            if not Path(path).exists():
                statuses.append(("MISSING FILE", path)); continue
            src = Path(path).read_text()
            block = extract_fn_block(src, "parse_arg")
            if not block:
                statuses.append(("info", f"{path}: no fn parse_arg (already migrated?)"))
                continue
            body_norm = normalize_body(block[2])
            sig_match = re.search(r'fn\s+parse_arg\s*\(([^)]*)\)', block[2])
            sig = sig_match.group(1).strip() if sig_match else "?"
            if body_norm != canon_norm or "flag: &str" in sig:
                statuses.append(
                    ("SKIPPED", f"{path}: body differs from canonical or signature is singular — migrate manually")
                )
                continue
            # Delete the fn block and add the use line.
            start, end, _ = block
            new_src = src[:start] + src[end:]
            new_src = insert_use_line(new_src, "nexus_cli_utils::parse_arg")
            if path not in files:
                files[path] = [src, new_src]
                statuses.append(("will apply", f"Finding 6: extract parse_arg from {path}"))
            else:
                # File already edited for Finding 7; apply on top of that.
                orig, current = files[path]
                blk2 = extract_fn_block(current, "parse_arg")
                if blk2:
                    s2, e2, _ = blk2
                    cur2 = current[:s2] + current[e2:]
                    cur2 = insert_use_line(cur2, "nexus_cli_utils::parse_arg")
                    files[path][1] = cur2
                    statuses.append(("will apply", f"Finding 6: extract parse_arg from {path} (after F7)"))

            # Ensure Cargo.toml dependency exists (Finding 6 apps may not have
            # been touched by Finding 7; symbol_graph for example).
            app_cargo = Path(path).parent.parent / "Cargo.toml"
            if app_cargo.exists():
                cargo_txt = app_cargo.read_text()
                if "nexus_cli_utils" not in cargo_txt:
                    dep_line = 'nexus_cli_utils = { path = "../../crates/nexus_cli_utils" }\n'
                    if "[dependencies]" in cargo_txt:
                        m = re.search(r'\[dependencies\]\n', cargo_txt)
                        files[str(app_cargo)] = [
                            cargo_txt,
                            cargo_txt[:m.end()] + dep_line + cargo_txt[m.end():],
                        ]
                    else:
                        files[str(app_cargo)] = [
                            cargo_txt,
                            cargo_txt.rstrip() + "\n\n[dependencies]\n" + dep_line,
                        ]
                    statuses.append(("will apply", f"add dep in {app_cargo}"))

# ─────────────────────────────────────────────────────────────────────
# Report
# ─────────────────────────────────────────────────────────────────────
print(); print("=" * 72); print("Edit summary"); print("=" * 72)
bad = False
for kind, msg in statuses:
    print(f"  [{kind:^16}] {msg}")
    if kind in ("PREIMAGE NOT FOUND", "MISSING FILE", "WARNING"):
        bad = True
if bad:
    print("\nSome warnings present — review before --apply.", file=sys.stderr)

print()
if APPLY:
    print("=" * 72); print("Writing files"); print("=" * 72)
    for p, (o, f) in files.items():
        if o != f:
            Path(p).parent.mkdir(parents=True, exist_ok=True)
            Path(p).write_text(f)
            print(f"  wrote {p}")
else:
    print("=" * 72); print("Dry-run — unified diff (pass --apply to write)"); print("=" * 72)
    for p, (o, f) in files.items():
        if o == f: continue
        if o is None:
            print(f"\n--- /dev/null\n+++ b/{p}")
            for line in f.splitlines(keepends=True):
                print("+" + line.rstrip("\n"))
        else:
            sys.stdout.writelines(difflib.unified_diff(
                o.splitlines(keepends=True), f.splitlines(keepends=True),
                fromfile=f"a/{p}", tofile=f"b/{p}"))

sys.exit(0 if not bad else 0)  # warnings don't block --apply; user reviews
PY

if [ "$APPLY" = "1" ] && [ "$CHECK" = "1" ]; then
    echo
    echo "======================================================================"
    echo "Running cargo check --workspace --all-targets"
    echo "======================================================================"
    RUSTFLAGS="-D warnings" cargo check --workspace --all-targets 2>&1 | tail -40
    echo "exit: ${PIPESTATUS[0]}"
fi
