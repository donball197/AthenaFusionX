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
echo "=== strategy pinning release (answer-delivery mandate) | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

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

CE_HELPERS_ANCHOR = '''fn detect_read_only(text: &str) -> bool {'''

CE_HELPERS_NEW = '''/// True when reconnaissance is satisfied and no deliverable file remains
/// outstanding: an answer-delivery mission whose only remaining work is
/// producing the response itself.
pub(crate) fn answer_delivery_pending(recon_ready: bool, unwritten_targets: &[&str]) -> bool {
    recon_ready && unwritten_targets.is_empty()
}

/// Phase mandate for answer-delivery missions.
///
/// STRATEGY PINNING RELEASE: once the reconnaissance contract is satisfied
/// and no artifact remains, re-selecting the satisfied search/read strategy
/// only thrashes (anti-loop filters every repeat and the budget burns).
/// The strategist must be told the physical work is DONE and the answer is
/// the deliverable. Wording deliberately avoids mutation verbs so this text
/// can never mint write/patch intents if mission text is ever re-inferred.
pub(crate) fn answer_delivery_mandate(mission: &str) -> String {
    format!(
        "{}\\n\\n[PHASE MANDATE: RECONNAISSANCE VERIFIED - DELIVER ANSWER]\\nThe reconnaissance contract is satisfied and no deliverable file is outstanding. Do NOT repeat completed actions. Produce the final answer in the mission's required output format and halt.",
        mission
    )
}

''' + CE_HELPERS_ANCHOR

CL_BRANCH_OLD = '''        } else if patch_function_strategy && critic_veto_present {'''

CL_BRANCH_NEW = '''        } else if crate::completion_engine::answer_delivery_pending(
            recon_ready,
            &unwritten_targets,
        ) {
            crate::completion_engine::answer_delivery_mandate(mission)
        } else if patch_function_strategy && critic_veto_present {'''

TEST_ANCHOR = '''    #[test]
    fn mission2_shape_reads_satisfied_once_the_command_is_evidenced() {'''

TEST_INSERT = '''    #[test]
    fn answer_delivery_pending_requires_recon_and_zero_artifacts() {
        assert!(super::answer_delivery_pending(true, &[]));
        assert!(!super::answer_delivery_pending(true, &["out.md"]));
        assert!(!super::answer_delivery_pending(false, &[]));
    }

    #[test]
    fn answer_delivery_mandate_directs_answer_not_recon() {
        let mandate = super::answer_delivery_mandate("audit the logic");
        assert!(mandate.contains("RECONNAISSANCE VERIFIED - DELIVER ANSWER"));
        assert!(mandate.contains("Do NOT repeat completed actions"));
        assert!(
            !mandate.contains("write_file"),
            "mandate must not carry mutation verbs: {mandate}"
        );
    }

''' + TEST_ANCHOR

# ====================== targets ==========================================
targets = [
    ("CE_HELPERS", CE, "completion_engine: answer-delivery helpers",
     CE_HELPERS_ANCHOR, CE_HELPERS_NEW,
     "fn answer_delivery_pending(", "fn answer_delivery_pending(",
     "fn detect_read_only(", 5),
    ("CL_BRANCH", CL, "cortical_loop: answer-delivery phase mandate",
     CL_BRANCH_OLD, CL_BRANCH_NEW,
     "answer_delivery_pending(", "answer_delivery_pending(",
     "let enhanced_mission = if write_phase_ready {", 22),
    ("CE_TESTS", CE, "completion_engine: answer-delivery tests",
     TEST_ANCHOR, TEST_INSERT,
     "fn answer_delivery_pending_requires_recon_and_zero_artifacts",
     "fn answer_delivery_pending_requires_recon_and_zero_artifacts",
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
    print(f"[STATUS] {key:10s} {label:52s} : {status}")

for rel, label, text in (
    (CE, "case_8 regression", "fn case_8_search_code_plus_search_memory_without_reads_preserves_both()"),
    (CE, "read-only regression suite", "fn root_context_output_labels_do_not_mint_mutation_conditions"),
    (CE, "detect_read_only", "fn detect_read_only("),
    (CL, "write_phase_ready", "fn write_phase_ready("),
    (CL, "enhanced_mission construction", "let enhanced_mission = if write_phase_ready {"),
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

# ------------------------- survey (read-only) -----------------------------
print()
print("[SURVEY] selection-side regions for the hard pinning fix (read-only):")
for probe in ("Exploring Strategy", "POST-SEARCH EXPANSION"):
    dump_region(CL, probe, 18)

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
    echo "[DRY-RUN] complete. Review the status lines, survey, and diff above."
else
    echo "[OK] answer-delivery pinning release applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib completion_engine::tests::answer_delivery"
    echo ">> Full suite:"
    echo "     cargo test -p nexus_runtime_core"
fi
