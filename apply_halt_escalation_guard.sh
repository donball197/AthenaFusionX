#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
elif [[ -n "${1:-}" ]]; then
    echo "[ERROR] unknown argument: $1" >&2
    exit 2
fi

WORKSPACE_ROOT="$(realpath -m "${WORKSPACE_ROOT:-$(pwd)}")"
echo "=== HALT escalation guard | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

CE="crates/runtime_core/src/completion_engine.rs"
CL="crates/runtime_core/src/cortical_loop.rs"
for f in "$CE" "$CL"; do
    if [[ ! -f "$WORKSPACE_ROOT/$f" ]]; then
        echo "[PREFLIGHT FAILED] missing $f" >&2
        exit 1
    fi
done

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
CE_REL="$CE" CL_REL="$CL" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
CE   = os.environ["CE_REL"]
CL   = os.environ["CL_REL"]

files = {rel: (root / rel, (root / rel).read_text(encoding="utf-8")) for rel in (CE, CL)}
patched = {rel: files[rel][1] for rel in files}

# ============================ anchors =====================================

CE_HELPER_ANCHOR = '''fn detect_read_only(text: &str) -> bool {'''

CE_HELPER_NEW = '''/// Session-scoped count of rejected halts at which the loop escalates.
/// Three rejected halts with no contract progress is the unsatisfiable-
/// contract deadlock signature (confirmed: 52-turn burn on a mission whose
/// answer was produced at turn 6). Fail loudly instead of burning budget.
pub(crate) const HALT_REJECTION_ESCALATION_THRESHOLD: usize = 3;

/// Number of `[HALT_REJECTED]` markers accumulated in the evidence log.
/// Total session count, not strictly consecutive: three rejected halts are
/// pathological regardless of interleaved evidence.
pub(crate) fn halt_rejection_count(evidence: &str) -> usize {
    evidence.matches("[HALT_REJECTED]").count()
}

''' + CE_HELPER_ANCHOR

CL_GUARD_OLD = '''                if is_halt_action && has_unwritten_artifacts {'''

CL_GUARD_NEW = r'''                if is_halt_action {
                    let rejections =
                        crate::completion_engine::halt_rejection_count(&session.local_evidence);
                    if rejections >= crate::completion_engine::HALT_REJECTION_ESCALATION_THRESHOLD {
                        // DEADLOCK ESCALATION: N rejected halts with no contract
                        // progress mean the contract is unsatisfiable (e.g. a
                        // prose-minted condition nothing can evidence). Fail
                        // loudly now instead of burning the remaining budget.
                        // Belief probe 1.0 matches condition_is_met's probing
                        // convention; the message is diagnostic only.
                        let unmet = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
                            &session.local_evidence,
                            1.0,
                            &session.contract.conditions,
                        );
                        self.log(format!(
                            "\x1b[1;31m>> [CORTEX ESCALATION] {} rejected HALTs with no contract progress. Aborting node. Unmet: {:?}\x1b[0m",
                            rejections, unmet
                        ));
                        return Err(anyhow::anyhow!(
                            "[HALT_ESCALATION] unsatisfiable completion contract after {} rejected halts; unmet: {:?}",
                            rejections, unmet
                        ));
                    }
                }

                if is_halt_action && has_unwritten_artifacts {'''

TEST_ANCHOR = '''    #[test]
    fn mission2_shape_reads_satisfied_once_the_command_is_evidenced() {'''

TEST_INSERT = '''    #[test]
    fn halt_rejection_count_counts_session_markers() {
        assert_eq!(super::halt_rejection_count(""), 0);
        assert_eq!(
            super::halt_rejection_count(
                "[READ_OK] a.rs\n[HALT_REJECTED] one\n[HALT_REJECTED] two\n"
            ),
            2
        );
    }

    #[test]
    fn halt_escalation_threshold_is_three() {
        assert_eq!(super::HALT_REJECTION_ESCALATION_THRESHOLD, 3);
    }

''' + TEST_ANCHOR

# ====================== targets ==========================================
targets = [
    ("CE_HELPER", CE, "completion_engine: halt rejection counter",
     CE_HELPER_ANCHOR, CE_HELPER_NEW,
     "fn halt_rejection_count(", "fn halt_rejection_count(",
     "fn detect_read_only(", 5),
    ("CL_GUARD", CL, "cortical_loop: deadlock escalation guard",
     CL_GUARD_OLD, CL_GUARD_NEW,
     "[CORTEX ESCALATION]", "[CORTEX ESCALATION]",
     "has_unwritten_artifacts", 25),
    ("CE_TESTS", CE, "completion_engine: escalation tests",
     TEST_ANCHOR, TEST_INSERT,
     "fn halt_rejection_count_counts_session_markers",
     "fn halt_rejection_count_counts_session_markers",
     "fn mission2_shape_reads_satisfied_once_the_command_is_evidenced", 5),
]

def dump_region(rel, probe, span):
    lines = files[rel][1].splitlines()
    idx = next((i for i, ln in enumerate(lines) if probe in ln), None)
    if idx is None:
        print(f"    [REGION DUMP FAILED] probe not found in {rel}: {probe!r}")
        return
    lo, hi = max(0, idx - span), min(len(lines), idx + span)
    print(f"    [REGION] {rel} around line {idx + 1} (probe: {probe!r}):")
    for i in range(lo, hi):
        print(f"      {i + 1:5d} | {lines[i]}")

plan = []
failures = []

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    text = patched[rel]
    old_count = text.count(old)
    if already in text:
        status = "SKIP (already present)"
        plan.append((key, rel, None))
    elif old_count == 1:
        status = "APPLY"
        plan.append((key, rel, (old, new)))
    elif old_count > 1:
        status = f"FAIL (anchor x{old_count}, want exactly 1)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    else:
        status = "FAIL (anchor not found)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    print(f"[STATUS] {key:9s} {label:52s} : {status}")

for rel, label, text in (
    (CE, "case_8 regression", "fn case_8_search_code_plus_search_memory_without_reads_preserves_both()"),
    (CE, "detect_read_only", "fn detect_read_only("),
    (CL, "write_phase_ready", "fn write_phase_ready("),
    (CL, "HALT diagnostics (Unmet detail)", "[HALT_REJECTED] Deterministic completion contract not satisfied. Unmet"),
):
    if text not in files[rel][1]:
        print(f"[INVARIANT VIOLATED] {label} is absent in {rel}")
        failures.append(("INVARIANT", rel, label, text, 5))

if failures:
    print()
    print("[DIAGNOSTIC] region dumps for failed targets:")
    for key, rel, label, region_probe, span in failures:
        print(f"  -- {key}: {label}")
        dump_region(rel, region_probe, span)
    print()
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    sys.exit(1)

# ------------------------- transformations -------------------------------
changed = []
for key, rel, edit in plan:
    if edit is None:
        continue
    old, new = edit
    patched[rel] = patched[rel].replace(old, new, 1)
    changed.append((key, rel))

if not changed:
    print()
    print("[NO-OP] every target is already in its final form. Nothing to write.")
    sys.exit(0)

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    if final_probe not in patched[rel]:
        print(f"[POST-PATCH FAILED] {label}: final probe absent: {final_probe!r}")
        sys.exit(1)

print()
print(f"[POST-PATCH OK] {len(changed)} target(s) transformed: {', '.join(k for k, _ in changed)}")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
for rel in (CE, CL):
    if patched[rel] != files[rel][1]:
        sys.stdout.writelines(
            difflib.unified_diff(
                files[rel][1].splitlines(keepends=True),
                patched[rel].splitlines(keepends=True),
                fromfile=f"a/{rel}",
                tofile=f"b/{rel}",
            )
        )

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

for rel in (CE, CL):
    if patched[rel] != files[rel][1]:
        files[rel][0].write_text(patched[rel], encoding="utf-8")
        print(f"[PATCH_OK] {rel}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review the status lines and diff above."
else
    echo "[OK] HALT escalation guard applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib completion_engine::tests::halt_"
    echo ">> Full suite:"
    echo "     cargo test -p nexus_runtime_core"
fi
