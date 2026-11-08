#!/bin/bash
# mutation_halt_fix_v2.sh
# Inserts a "mutation-complete → halt" branch that fires on write-only
# contracts (where reconnaissance_complete returns false because there's
# nothing recon to satisfy).
# Idempotent. Backs up. Compiles check.
set -euo pipefail

STRATEGIZE="crates/runtime_core/src/cortical_loop/strategize.rs"
BACKUP="${STRATEGIZE}.pre_mutation_halt_v2.bak"

[ -f "$STRATEGIZE" ] || { echo "ERROR: run from repo root" >&2; exit 1; }

echo "════════════════════════════════════════════════════════════"
echo "  MUTATION-HALT FIX v2"
echo "════════════════════════════════════════════════════════════"

if [ ! -f "$BACKUP" ]; then
    cp "$STRATEGIZE" "$BACKUP"
    echo "[1/3] Backup -> $BACKUP"
else
    echo "[1/3] Backup exists, preserving"
fi

if grep -q "Mutation complete; forcing" "$STRATEGIZE"; then
    echo "[2/3] Patch already applied — skipping"
else
    python3 - <<'PYEOF'
import sys

path = "crates/runtime_core/src/cortical_loop/strategize.rs"
src = open(path, encoding="utf-8").read()

anchor = '''            strategy
        } else if session.coverage.coverage_pct() >= 99.9'''

if src.count(anchor) != 1:
    print(f"[2/3] ERROR: anchor not unique (count={src.count(anchor)})", file=sys.stderr)
    sys.exit(1)

# Raw string: \x1b and \n stay as Rust escapes (backslash + chars),
# which is exactly what the file needs.
new_block = r'''            strategy
        } else if session.contract.is_mutation_task
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && !halt_rejected_since_last_evidence(&session.local_evidence)
            && {
                // If the contract carries ONLY write conditions (no reads,
                // searches, or belief thresholds), skip the recon check --
                // `reconnaissance_complete` returns false for a write-only
                // contract because there are no recon conditions to satisfy.
                // Otherwise, require the same recon_complete guard the
                // read-halt branch below uses.
                let only_write_conditions = session.contract.conditions.iter().all(|c| {
                    matches!(
                        c,
                        crate::completion_engine::CompletionCondition::WritePathAcquired { .. }
                            | crate::completion_engine::CompletionCondition::WriteMarkerAcquired
                    )
                });
                only_write_conditions
                    || crate::completion_engine::SubgoalCompletionEngine::reconnaissance_complete(
                        &session.contract,
                        &session.local_evidence,
                        1.0,
                    )
            }
        {
            // MUTATION COMPLETE -- mutation-phase analog of the read-halt
            // branch below. A write-only contract (e.g. `WritePathAcquired`
            // with no reads or searches) has no recon conditions, so
            // `reconnaissance_complete` cannot fire the read-halt branch.
            // Without this branch, the loop rewrites the same artifact every
            // turn until the cognitive-turn budget exhausts.
            let strategy = "halt".to_string();
            self.log(
                "\x1b[1;35m>> [CORTEX PHASE TRANSITION]\x1b[0m Mutation complete; forcing `halt` strategy for synthesis."
                    .to_string(),
            );
            session.local_evidence.push_str(
                "\n[NODE PHASE] Your node's mutation criteria are satisfied. The required next action is `halt`, and the halt text must summarize THIS node's findings only. Do NOT reference any other node's deliverable.\n"
            );
            strategy
        } else if session.coverage.coverage_pct() >= 99.9'''

src = src.replace(anchor, new_block, 1)
open(path, "w", encoding="utf-8").write(src)
print("[2/3] Patch applied")
PYEOF
fi

echo "[3/3] cargo check -p nexus_runtime_core..."
set +e
cargo check -p nexus_runtime_core 2>&1 | tail -8
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
echo "Branch order in the strategist's phase-transition chain:"
grep -n "if write_phase_ready\|Mutation complete; forcing\|Reconnaissance complete; forcing\|h_content.clone()" "$STRATEGIZE" | head -6
echo
echo "Review the diff:"
echo "    git diff $STRATEGIZE"
echo
echo "Next run: node 4 should"
echo "  turn 2: [WRITE_OK] full_system_audit.md"
echo "  turn 3: >> [CORTEX PHASE TRANSITION] Mutation complete; forcing \`halt\`"
echo "  turn 4-5: [HALT] synthesized by strategist, node verified"
echo "  final: [DAG COMPLETE] All 4/4"
echo
echo "To revert:"
echo "    cp $BACKUP $STRATEGIZE"
echo "════════════════════════════════════════════════════════════"
