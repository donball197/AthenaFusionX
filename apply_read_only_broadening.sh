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
echo "=== read_only broadening | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

CE="crates/runtime_core/src/completion_engine.rs"
if [[ ! -f "$WORKSPACE_ROOT/$CE" ]]; then
    echo "[PREFLIGHT FAILED] missing $CE" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
CE_REL="$CE" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
CE   = os.environ["CE_REL"]

path     = root / CE
original = path.read_text(encoding="utf-8")

READ_ONLY_OLD = '''        let read_only = classify_text.contains("read-only")
            || classify_text.contains("read only")
            || classify_text.contains("audit only")
            || classify_text.contains("do not modify any source files");'''

READ_ONLY_NEW = '''        let read_only = classify_text.contains("read-only")
            || classify_text.contains("read only")
            || classify_text.contains("audit only")
            || classify_text.contains("do not modify any source files")
            || classify_text.contains("do not modify any files")
            || classify_text.contains("do not modify")
            || classify_text.contains("without modifying")
            || classify_text.contains("must not modify");'''

TEST_ANCHOR = '''    #[test]
    fn non_read_only_mission_preserves_write_contract() {'''

TEST_INSERT = '''    #[test]
    fn read_only_detects_do_not_modify_any_files_phrase() {
        let mission = "OBJECTIVE: Perform a deterministic code audit of the \\
                       following Rust logic.\\n\\n\\
                       Do not modify any files.\\n\\n\\
                       MANDATORY NODE SUCCESS CRITERIA:\\n\\
                       SEARCH_CODE_OK:crates/runtime_core/src:Evidence\\n\\n\\
                       NODE COMPLETION RULE: ...\\n\\n\\
                       ROOT DIRECTIVE CONTEXT:\\n\\
                       Then provide exactly this output format:\\n\\
                       ROOT_CAUSE: <both independent defects>\\n\\
                       TRACE: <...>\\n\\
                       FIX: <the minimal logical correction>";

        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(
            contract.read_only,
            "read_only must be detected from 'Do not modify any files'; conditions = {:?}",
            contract.conditions
        );
        assert!(!contract.is_mutation_task, "read-only mission must not be a mutation task");
        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::PatchMarkerAcquired
            )),
            "read-only mission must not require patch evidence; conditions = {:?}",
            contract.conditions
        );
        assert!(
            contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::SearchTermAcquired { term }
                    if term == "crates/runtime_core/src:Evidence"
            )),
            "SEARCH_CODE_OK:crates/runtime_core/src:Evidence must survive; conditions = {:?}",
            contract.conditions
        );
    }

    #[test]
    fn read_only_detects_without_modifying_phrase() {
        let mission = "OBJECTIVE: audit without modifying any files.\\n\\
                       SEARCH_CODE_OK:foo";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(contract.read_only, "read_only must be detected from 'without modifying'");
        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::PatchMarkerAcquired
            )),
            "read-only mission must not require patch evidence"
        );
    }

    #[test]
    fn non_read_only_mission_preserves_write_contract() {'''

PATH_BOUND_INVARIANT = '''        let has_explicit_path_bound = objective.contains("READ_OK:")
            || objective.contains("WRITE_OK:")
            || objective.contains("SEARCH_CODE_OK:")
            || objective.contains("SEARCH_MEMORY_OK:")
            || objective.contains("PATCH_FUNCTION_OK:")
            || objective.contains("READ_FUNCTION_OK:");'''

CASE_8_INVARIANT = "fn case_8_search_code_plus_search_memory_without_reads_preserves_both()"

problems = []

ro_count = original.count(READ_ONLY_OLD)
if ro_count == 0:
    if READ_ONLY_NEW in original:
        problems.append("READ_ONLY ALREADY APPLIED: broadening is already present")
    else:
        problems.append("READ_ONLY ANCHOR NOT FOUND: expected pre-patch read_only assignment is absent")
elif ro_count > 1:
    problems.append(f"READ_ONLY ANCHOR x{ro_count} (want exactly 1)")

ta_count = original.count(TEST_ANCHOR)
if ta_count == 0:
    problems.append("TEST ANCHOR NOT FOUND: non_read_only_mission_preserves_write_contract opening is absent")
elif ta_count > 1:
    problems.append(f"TEST ANCHOR x{ta_count} (want exactly 1)")

for name in (
    "read_only_detects_do_not_modify_any_files_phrase",
    "read_only_detects_without_modifying_phrase",
):
    if name in original:
        problems.append(f"TEST ALREADY PRESENT: {name}")

if PATH_BOUND_INVARIANT not in original:
    problems.append("INVARIANT VIOLATED: has_explicit_path_bound six-marker guard is absent")

if CASE_8_INVARIANT not in original:
    problems.append("INVARIANT VIOLATED: case_8 regression test is absent")

if problems:
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    for p in problems:
        print("  -", p)
    sys.exit(1)

print("[PREFLIGHT OK] 2 anchors matched exactly once")
print("[PREFLIGHT OK] prior fix invariants verified (path-bound guard + case_8)")

patched = original.replace(READ_ONLY_OLD, READ_ONLY_NEW, 1)
patched = patched.replace(TEST_ANCHOR, TEST_INSERT, 1)

if patched == original:
    print("[PREFLIGHT FAILED] no textual change produced (internal error)")
    sys.exit(1)

assert patched.count(READ_ONLY_NEW) == 1
assert patched.count(READ_ONLY_OLD) == 0
assert patched.count("read_only_detects_do_not_modify_any_files_phrase") == 1
assert patched.count("read_only_detects_without_modifying_phrase") == 1
assert PATH_BOUND_INVARIANT in patched
assert CASE_8_INVARIANT in patched

print("[POST-PATCH OK] read_only broadened, tests inserted, invariants held")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
sys.stdout.writelines(
    difflib.unified_diff(
        original.splitlines(keepends=True),
        patched.splitlines(keepends=True),
        fromfile=f"a/{CE}",
        tofile=f"b/{CE}",
    )
)

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Preflight, transformation, and post-patch validation passed.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

path.write_text(patched, encoding="utf-8")
print()
print(f"[PATCH_OK] {CE}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review the diff above."
else
    echo "[OK] read_only broadening applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib completion_engine::tests::read_only_detects"
    echo ">> Full suite:"
    echo "     cargo test -p nexus_runtime_core"
fi
