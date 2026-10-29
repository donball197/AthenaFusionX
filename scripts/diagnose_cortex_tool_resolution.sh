#!/usr/bin/env bash
# 01
set -Eeuo pipefail

# 02
# AthenaFusionX Cortex tool-resolution diagnostic
# 03
# This script does not modify source files.
# 04
# It identifies the actual implementations that must be patched.

# 05
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

# 06
REPORT="${ROOT}/cortex_tool_resolution_report.txt"
: > "$REPORT"

# 07
log() {
    printf '%s\n' "$*" | tee -a "$REPORT"
}

# 08
log "=================================================="
log "AthenaFusionX Cortex Tool Resolution Diagnostic"
log "=================================================="
log "Repository: $ROOT"
log "Date: $(date -Is)"
log ""

# 09
log "[01] Workspace packages"
log "--------------------------------------------------"
cargo metadata --no-deps --format-version 1 2>&1 | tee -a "$REPORT" || true
log ""

# 10
log "[02] AST package and binary discovery"
log "--------------------------------------------------"

find "$ROOT" \
    \( -path '*/target/*' -o -path '*/.git/*' \) -prune -o \
    \( -name 'Cargo.toml' -o -name 'nexus_ast_tool' -o -name 'nexus-ast-tool' \) \
    -print 2>/dev/null | tee -a "$REPORT"

log ""

# 11
log "[03] Actual AST executable candidates"
log "--------------------------------------------------"

for candidate in \
    "$ROOT/target/debug/nexus-ast-tool" \
    "$ROOT/target/release/nexus-ast-tool" \
    "$ROOT/target/debug/nexus-ast-tool" \
    "$ROOT/target/release/nexus-ast-tool"
do
    if [[ -x "$candidate" ]]; then
        log "FOUND executable: $candidate"
        "$candidate" --help 2>&1 | head -80 | tee -a "$REPORT" || true
    else
        log "NOT FOUND: $candidate"
    fi
done

log ""

# 12
log "[04] Workspace package invocation test"
log "--------------------------------------------------"

if cargo metadata --no-deps --format-version 1 \
    | grep -q '"name":"nexus_ast_tool"'
then
    log "FOUND Cargo package: nexus_ast_tool"
    log "Correct package invocation form:"
    log "cargo run -p nexus_ast_tool -- audit ..."
else
    log "WARNING: Cargo package nexus_ast_tool was not found by metadata"
fi

log ""

# 13
log "[05] Locate command generation and execution code"
log "--------------------------------------------------"

SEARCH_ROOTS=(
    "$ROOT/crates"
    "$ROOT/apps"
    "$ROOT/src"
)

for dir in "${SEARCH_ROOTS[@]}"; do
    [[ -d "$dir" ]] || continue

    grep -RInE \
        --exclude-dir=target \
        --exclude-dir=.git \
        --include='*.rs' \
        --include='*.toml' \
        --include='*.md' \
        'execute_command|MotorCommand|RoboticExecutor|STRATEGIST PROPOSAL|nexus_ast_tool|command not found|ACTION FAILED|DETERMINISTIC GUARD|Valid Halt|CortexOutcome' \
        "$dir" 2>/dev/null | tee -a "$REPORT" || true
done

log ""

# 14
log "[06] Locate evidence and halt implementation"
log "--------------------------------------------------"

grep -RInE \
    --exclude-dir=target \
    --exclude-dir=.git \
    --include='*.rs' \
    'EVIDENCE ACQUIRED|CMD_OK|ACTION FAILED|exit status|stderr|stdout|halt|CompletionProposed|Completion|TurnsExhausted' \
    "$ROOT/crates" "$ROOT/apps" 2>/dev/null | tee -a "$REPORT" || true

log ""

# 15
log "[07] Locate existing tests"
log "--------------------------------------------------"

find "$ROOT" \
    \( -path '*/target/*' -o -path '*/.git/*' \) -prune -o \
    \( -iname '*test*' -o -iname '*spec*' \) -print 2>/dev/null \
    | tee -a "$REPORT"

log ""

# 16
log "[08] Required regression cases"
log "--------------------------------------------------"

cat <<'EOF' | tee -a "$REPORT"
CASE A:
  Bare nexus_ast_tool invocation must not be authorized
  unless executable resolution confirms it exists in PATH.

CASE B:
  A workspace package must resolve to:
  cargo run -p nexus_ast_tool -- <arguments>
  or an existing absolute/relative binary path.

CASE C:
  A target path must be verified before execution.

CASE D:
  Failed execution must create fresh failure evidence.

CASE E:
  Previous successful evidence must not be reused as proof
  that a new command succeeded.

CASE F:
  A PASS halt must require evidence supporting the actual
  requested claim, not merely a valid final_answer JSON object.

CASE G:
  The same failed command must not be proposed again
  without a documented correction.
EOF

log ""
log "=================================================="
log "Report written to: $REPORT"
log "=================================================="
