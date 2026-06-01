#!/usr/bin/env bash
# =============================================================================
# fix_errors.sh — One-shot surgical repair for nexus_runtime_core
# Fixes all 9 errors reported by: cargo clippy --workspace --all-targets -- -D warnings
# =============================================================================
set -euo pipefail

REPO="${1:-$HOME/nexus_singularity-dev}"
cd "$REPO"

RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
ok()   { echo -e "${GREEN}[FIX]${RESET}  $*"; }
info() { echo -e "${CYAN}[INFO]${RESET} $*"; }
die()  { echo -e "${RED}[FAIL]${RESET} $*" >&2; exit 1; }

echo -e "\n${BOLD}════════════════════════════════════════════${RESET}"
echo -e "${BOLD} NEXUS Surgical Patch — 9-Error Blitz${RESET}"
echo -e "${BOLD}════════════════════════════════════════════${RESET}\n"

# ---------------------------------------------------------------------------
# FIX 1 & 2 — scout_agent.rs and shell_agent.rs
# SubstrateContext lives in agents::agentic_runtime, not agents directly.
# ---------------------------------------------------------------------------

info "Fix 1/6 — scout_agent.rs: wrong SubstrateContext import path"
python3 - << 'PY'
import re, pathlib
p = pathlib.Path("crates/runtime_core/src/agents/scout_agent.rs")
old = "use crate::agents::{LogicAgent, SubstrateContext};"
new = "use crate::agents::{LogicAgent, agentic_runtime::SubstrateContext};"
txt = p.read_text()
assert old in txt, f"Pattern not found in {p}"
p.write_text(txt.replace(old, new, 1))
print(f"  patched {p}")
PY
ok "scout_agent.rs import fixed"

info "Fix 2/6 — shell_agent.rs: wrong SubstrateContext import path"
python3 - << 'PY'
import pathlib
p = pathlib.Path("crates/runtime_core/src/agents/shell_agent.rs")
old = "use crate::agents::{LogicAgent, SubstrateContext};"
new = "use crate::agents::{LogicAgent, agentic_runtime::SubstrateContext};"
txt = p.read_text()
assert old in txt, f"Pattern not found in {p}"
p.write_text(txt.replace(old, new, 1))
print(f"  patched {p}")
PY
ok "shell_agent.rs import fixed"

# ---------------------------------------------------------------------------
# FIX 3 — agents/mod.rs
# SubstrateContext re-export: the use statement references an item that isn't
# pub in agentic_runtime. Remove the stray use — callers import it directly.
# ---------------------------------------------------------------------------

info "Fix 3/6 — agents/mod.rs: remove broken SubstrateContext re-export"
python3 - << 'PY'
import pathlib
p = pathlib.Path("crates/runtime_core/src/agents/mod.rs")
old = "use crate::agents::agentic_runtime::SubstrateContext;\n"
txt = p.read_text()
assert old in txt, f"Pattern not found in {p}"
p.write_text(txt.replace(old, "", 1))
print(f"  patched {p}")
PY
ok "agents/mod.rs stray import removed"

# ---------------------------------------------------------------------------
# FIX 4 — task_manager.rs
# RoboticExecutor::new() signature changed: it no longer takes api_key.
# The second arg is tx which we have; remove the stale api_key.to_string().
# ---------------------------------------------------------------------------

info "Fix 4/6 — task_manager.rs: remove stale api_key arg from RoboticExecutor::new()"
python3 - << 'PY'
import pathlib
p = pathlib.Path("crates/runtime_core/src/task_manager.rs")
old = "    let mut executor = RoboticExecutor::new(api_key.to_string(), tx.clone());"
new = "    let mut executor = RoboticExecutor::new(tx.clone());"
txt = p.read_text()
assert old in txt, f"Pattern not found in {p}"
p.write_text(txt.replace(old, new, 1))
print(f"  patched {p}")
PY
ok "task_manager.rs RoboticExecutor::new() call fixed"

# ---------------------------------------------------------------------------
# FIX 5 — wasm_forge.rs
# Unused import: `use std::process::Command;`  (tokio::process::Command is
# used instead). Suppress with #[allow] rather than deleting in case it
# reappears; simpler to just remove the dead import.
# ---------------------------------------------------------------------------

info "Fix 5/6 — wasm_forge.rs: remove unused std::process::Command import"
python3 - << 'PY'
import pathlib
p = pathlib.Path("crates/runtime_core/src/wasm_forge.rs")
old = "use std::process::Command;\n"
txt = p.read_text()
assert old in txt, f"Pattern not found in {p}"
p.write_text(txt.replace(old, "", 1))
print(f"  patched {p}")
PY
ok "wasm_forge.rs unused import removed"

# ---------------------------------------------------------------------------
# FIX 6 — executor.rs (3 errors in one file)
#   E0107: Result<Vec<String>> and Result<String> need the second type param.
#          These are anyhow::Result which IS single-arg — the issue is that
#          std::result::Result leaked into scope. Confirm anyhow is imported.
#   E0609/E0282: `.awaitpath` is a typo leftover — should be `.await` then
#          the path is already in scope. The offending line is a double-write
#          that was never cleaned up.
# ---------------------------------------------------------------------------

info "Fix 6/6 — executor.rs: fix Result type params + .awaitpath typo"
python3 - << 'PY'
import pathlib, re

p = pathlib.Path("crates/runtime_core/src/executor.rs")
txt = p.read_text()

# ── Ensure anyhow::Result is imported so single-arg Result<T> compiles ──
if "use anyhow::Result;" not in txt and "use anyhow::{" not in txt:
    # Prepend the import after the first `use` block
    txt = "use anyhow::Result;\n" + txt
    print("  added anyhow::Result import")

# ── Fix .awaitpath double-write typo ──
# Bad line (from audit):
# tokio::fs::write(tokio::fs::write(&path, content).awaitpath, content).await...
bad = re.compile(
    r'tokio::fs::write\(\s*tokio::fs::write\(&path,\s*content\)\.awaitpath,\s*content\)'
    r'\.await\.map_err\(Into::into\)\.map\(\|_\|\s*format'
)
if bad.search(txt):
    # Replace entire bad expression with the correct single write + format
    txt = bad.sub(
        'tokio::fs::write(&path, content).await.map_err(anyhow::Error::from).map(|_| format',
        txt
    )
    print("  fixed .awaitpath double-write")
else:
    # Looser fallback: fix just the .awaitpath field access
    txt = txt.replace(".awaitpath", "")
    print("  removed stray .awaitpath token")

p.write_text(txt)
print(f"  patched {p}")
PY
ok "executor.rs errors fixed"

# ---------------------------------------------------------------------------
# Verify
# ---------------------------------------------------------------------------
echo ""
info "Running cargo clippy to verify…"
echo ""
if cargo clippy --workspace --all-targets -- -D warnings 2>&1; then
    echo ""
    echo -e "${GREEN}${BOLD}✓ All errors resolved. Workspace is clean.${RESET}"
else
    echo ""
    echo -e "${RED}Some errors remain — see output above.${RESET}"
    exit 1
fi
