#!/bin/bash
# phase_transition_inspect.sh — read-only, no writes, no backups needed
set -euo pipefail

STRATEGIZE="crates/runtime_core/src/cortical_loop/strategize.rs"
[ -f "$STRATEGIZE" ] || { echo "ERROR: run from repo root" >&2; exit 1; }

echo "════════════════════════════════════════════════════════════"
echo "  1. Phase transition block (lines 140-215, numbered)"
echo "════════════════════════════════════════════════════════════"
sed -n '140,215p' "$STRATEGIZE" | nl -ba -v140
echo

echo "════════════════════════════════════════════════════════════"
echo "  2. Strategy enum variants referenced in this file"
echo "════════════════════════════════════════════════════════════"
grep -nE 'Strategy::|enum Strategy|StrategyKind|matches!\(.*Strategy' "$STRATEGIZE" | head -40
echo

echo "════════════════════════════════════════════════════════════"
echo "  3. Contract-completion checks in this file"
echo "════════════════════════════════════════════════════════════"
grep -nE 'satisfied|contract_met|check_contract|is_complete|fully_satisfied|all_conditions' "$STRATEGIZE" | head -30
echo

echo "════════════════════════════════════════════════════════════"
echo "  4. Function signatures in this file"
echo "════════════════════════════════════════════════════════════"
grep -nE '^\s*(pub(\([^)]*\))?\s+)?(async\s+)?fn ' "$STRATEGIZE" | head -30
echo

echo "Done. Paste all four sections back."
