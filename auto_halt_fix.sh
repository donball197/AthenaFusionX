#!/bin/bash
# auto_halt_fix.sh — patch auto_halt_report_aggregated to include [WRITE_OK]
# Idempotent. Backs up. Compiles.
set -euo pipefail

STRATEGIZE="crates/runtime_core/src/cortical_loop/strategize.rs"
BACKUP="${STRATEGIZE}.pre_auto_halt_fix.bak"

[ -f "$STRATEGIZE" ] || { echo "ERROR: run from repo root" >&2; exit 1; }

echo "════════════════════════════════════════════════════════════"
echo "  AUTO-HALT WRITE-EVIDENCE FIX"
echo "════════════════════════════════════════════════════════════"

if [ ! -f "$BACKUP" ]; then
    cp "$STRATEGIZE" "$BACKUP"
    echo "[1/3] Backup -> $BACKUP"
else
    echo "[1/3] Backup exists, preserving"
fi

if grep -q "write_markers.iter().take(5)" "$STRATEGIZE"; then
    echo "[2/3] Patch already applied — skipping"
else
    python3 - <<'PYEOF'
import sys

path = "crates/runtime_core/src/cortical_loop/strategize.rs"
src = open(path, encoding="utf-8").read()

# ── Patch A: guard + write-marker prepend ──────────────────────────
old_a = r'''    if ok.is_empty() {
        return "No physical command evidence was recorded for this node.".to_string();
    }
    let mut report = String::new();
    report.push_str(&format!(
        "Read-only audit findings: {} command(s) executed successfully during this node.\n\n",
        ok.len()
    ));'''

new_a = r'''    // Mutation contracts have no [CMD_OK] blocks, but they DO produce
    // [WRITE_OK] markers. Without surfacing those markers here, the auto-halt
    // for a write node looks "read-only" and the LLM mission verifier rejects
    // it for failing to prove the artifact landed.
    let write_markers: Vec<String> = evidence
        .lines()
        .filter(|l| l.trim_start().starts_with("[WRITE_OK]"))
        .map(|l| l.trim().to_string())
        .collect();
    let has_write = !write_markers.is_empty();

    if ok.is_empty() && !has_write {
        return "No physical command evidence was recorded for this node.".to_string();
    }
    let mut report = String::new();
    if has_write {
        for w in write_markers.iter().take(5) {
            report.push_str(w);
            report.push('\n');
        }
        report.push('\n');
    }
    if !ok.is_empty() {
        let header = if has_write { "Node findings" } else { "Read-only audit findings" };
        report.push_str(&format!(
            "{}: {} command(s) executed successfully during this node.\n\n",
            header,
            ok.len()
        ));
    }
    if has_write && ok.is_empty() {
        report.push_str("Deliverable artifact written. No additional commands were required.\n\n");
    }'''

if old_a not in src:
    print("[2/3] ERROR: Patch A anchor not found", file=sys.stderr)
    sys.exit(1)
src = src.replace(old_a, new_a, 1)

# ── Patch B: trailing sentence, conditional on has_write ───────────
old_b = r'''    report.push_str(
        "Each command above was recorded with a clean exit status in [CMD_OK] evidence.",
    );
    report
}'''

new_b = r'''    if ok.is_empty() {
        report.push_str("Every action above is physically evidenced in this session's [*_OK] markers.");
    } else {
        report.push_str("Each command above was recorded with a clean exit status in [CMD_OK] evidence.");
    }
    report
}'''

if old_b not in src:
    print("[2/3] ERROR: Patch B anchor not found", file=sys.stderr)
    sys.exit(1)
src = src.replace(old_b, new_b, 1)

open(path, "w", encoding="utf-8").write(src)
print("[2/3] Patch applied (2 hunks: write-marker prepend + trailing sentence)")
PYEOF
fi

echo "[3/3] cargo check -p nexus_runtime_core..."
set +e
cargo check -p nexus_runtime_core 2>&1 | tail -4
CHECK_EXIT=$?
set -e

if [ "$CHECK_EXIT" -ne 0 ]; then
    echo "════════════════════════════════════════════════════════════"
    echo "❌ COMPILE FAILED — REVERTING"
    cp "$BACKUP" "$STRATEGIZE"
    echo "Restored: $STRATEGIZE"
    exit 1
fi

echo
echo "════════════════════════════════════════════════════════════"
echo "✅ PATCH APPLIED"
echo "════════════════════════════════════════════════════════════"
echo
echo "New branch shape:"
grep -n "write_markers.iter().take(5)\|let has_write\|Node findings\|Read-only audit findings" "$STRATEGIZE" | head -8
echo
echo "Review:"
echo "    git diff $STRATEGIZE | grep -A40 auto_halt_report_aggregated"
echo
echo "Expected effect: node 4 turn 3 auto-halt will include"
echo "    [WRITE_OK] full_system_audit.md"
echo "as the first line of reasoning, which the mission verifier accepts."
echo
echo "Revert:"
echo "    cp $BACKUP $STRATEGIZE"
echo "════════════════════════════════════════════════════════════"
