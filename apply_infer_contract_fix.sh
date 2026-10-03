#!/usr/bin/env bash
# apply_infer_contract_fix.sh — one defect, one file, one transaction.
#
# Defect:
#   completion_engine::infer_contract's `has_explicit_path_bound` guard
#   only recognizes READ_OK:/WRITE_OK:. A criteria block containing:
#
#       SEARCH_CODE_OK:foo
#       SEARCH_MEMORY_OK:bar
#
#   therefore takes the SearchMemory early-return and silently drops the
#   SEARCH_CODE_OK: criterion before any condition is emitted.
#
# Fix:
#   Broaden the guard to recognize:
#     READ_OK:
#     WRITE_OK:
#     SEARCH_CODE_OK:
#     SEARCH_MEMORY_OK:
#     PATCH_FUNCTION_OK:
#     READ_FUNCTION_OK:
#
# Guarantees:
#   * exactly one Python process performs read -> preflight -> diff -> write
#   * every pre-patch anchor must occur exactly once
#   * already-applied/drifted source fails closed
#   * dry-run performs zero writes
#   * all transformation happens in memory before the first write
#   * final write uses temp-file + fsync + os.replace
#   * no unrelated files are modified
#
# Usage:
#   ./apply_infer_contract_fix.sh --dry-run
#   ./apply_infer_contract_fix.sh
#
# Tests after application:
#   cargo test -p nexus_runtime_core --lib completion_engine::tests::case_8
#   cargo test -p nexus_runtime_core

set -euo pipefail

DRY_RUN=false

if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
elif [[ -n "${1:-}" ]]; then
    echo "[ERROR] unknown argument: $1" >&2
    echo "Usage: $0 [--dry-run]" >&2
    exit 2
fi

WORKSPACE_ROOT="$(realpath -m "${WORKSPACE_ROOT:-$(pwd)}")"
CE="crates/runtime_core/src/completion_engine.rs"

echo "=== infer_contract path-bound guard fix | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

if [[ ! -f "$WORKSPACE_ROOT/$CE" ]]; then
    echo "[PREFLIGHT FAILED] missing $CE" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
CE_REL="$CE" \
python3 - <<'PYEOF'
import difflib
import os
import pathlib
import sys
import tempfile

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry = os.environ["NEXUS_DRY"] == "1"
ce_rel = os.environ["CE_REL"]

path = root / ce_rel

# ================================================================
# READ
# ================================================================

original = path.read_text(encoding="utf-8")

# ================================================================
# EXACT PRE-PATCH ANCHORS
# ================================================================

GUARD_OLD = """        let has_explicit_path_bound =
            objective.contains("READ_OK:") || objective.contains("WRITE_OK:");"""

GUARD_NEW = """        let has_explicit_path_bound = objective.contains("READ_OK:")
            || objective.contains("WRITE_OK:")
            || objective.contains("SEARCH_CODE_OK:")
            || objective.contains("SEARCH_MEMORY_OK:")
            || objective.contains("PATCH_FUNCTION_OK:")
            || objective.contains("READ_FUNCTION_OK:");"""

TEST_ANCHOR = '''    #[test]
    fn case_4_reads_plus_all_criteria_preserve_everything() {'''

TEST_NAME = "case_8_search_code_plus_search_memory_without_reads_preserves_both"

TEST_INSERT = '''    #[test]
    fn case_8_search_code_plus_search_memory_without_reads_preserves_both() {
        // Regression: when SEARCH_CODE_OK: and SEARCH_MEMORY_OK: are both
        // present without READ_OK:/WRITE_OK:, infer_contract must not take
        // the SearchMemory early-return and silently discard SEARCH_CODE_OK:.
        let mission = "SEARCH_CODE_OK:foo\\nSEARCH_MEMORY_OK:bar";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(
            contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::SearchTermAcquired { term } if term == "foo"
            )),
            "SEARCH_CODE_OK:foo must survive; conditions = {:?}",
            contract.conditions
        );

        assert!(
            contract
                .conditions
                .iter()
                .any(|c| matches!(
                    c,
                    CompletionCondition::SearchMemoryMarkerAcquired
                )),
            "SEARCH_MEMORY_OK must survive; conditions = {:?}",
            contract.conditions
        );
    }

    #[test]
    fn case_4_reads_plus_all_criteria_preserve_everything() {'''

# ================================================================
# PREFLIGHT
# ================================================================

problems = []

guard_count = original.count(GUARD_OLD)

if guard_count == 0:
    if GUARD_NEW in original:
        problems.append(
            "GUARD ALREADY APPLIED: has_explicit_path_bound already contains "
            "all six path-bound markers; refusing to re-apply"
        )
    else:
        problems.append(
            "GUARD ANCHOR NOT FOUND: expected pre-patch "
            "has_explicit_path_bound guard is absent; source may have drifted"
        )
elif guard_count != 1:
    problems.append(
        f"GUARD ANCHOR x{guard_count} (want exactly 1): "
        "has_explicit_path_bound anchor is not unique"
    )

test_count = original.count(TEST_ANCHOR)

if test_count == 0:
    if TEST_NAME in original:
        problems.append(
            f"TEST ALREADY INSERTED: {TEST_NAME} is already present; "
            "refusing to duplicate"
        )
    else:
        problems.append(
            "TEST ANCHOR NOT FOUND: expected case_4 test opening is absent; "
            "source may have drifted"
        )
elif test_count != 1:
    problems.append(
        f"TEST ANCHOR x{test_count} (want exactly 1): "
        "case_4 test opening is not unique"
    )

if problems:
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    for problem in problems:
        print(f"  - {problem}")
    sys.exit(1)

print("[PREFLIGHT OK] 2 anchors matched exactly once")

# ================================================================
# TRANSFORM IN MEMORY
# ================================================================

patched = original.replace(GUARD_OLD, GUARD_NEW, 1)
patched = patched.replace(TEST_ANCHOR, TEST_INSERT, 1)

if patched == original:
    print("[PREFLIGHT FAILED] transformation produced no change")
    sys.exit(1)

# ================================================================
# POST-PATCH INVARIANTS
# ================================================================

post_errors = []

if patched.count(GUARD_NEW) != 1:
    post_errors.append(
        "post-patch guard does not occur exactly once"
    )

if GUARD_OLD in patched:
    post_errors.append(
        "pre-patch guard still exists after transformation"
    )

if patched.count(TEST_NAME) != 1:
    post_errors.append(
        f"regression test {TEST_NAME} does not occur exactly once"
    )

required_markers = (
    'objective.contains("READ_OK:")',
    'objective.contains("WRITE_OK:")',
    'objective.contains("SEARCH_CODE_OK:")',
    'objective.contains("SEARCH_MEMORY_OK:")',
    'objective.contains("PATCH_FUNCTION_OK:")',
    'objective.contains("READ_FUNCTION_OK:")',
)

for marker in required_markers:
    if marker not in patched:
        post_errors.append(
            f"post-patch guard missing required marker: {marker}"
        )

if post_errors:
    print("[POST-PATCH VALIDATION FAILED] ZERO FILES MODIFIED.")
    for error in post_errors:
        print(f"  - {error}")
    sys.exit(1)

print("[POST-PATCH OK] guard and regression test validated")

# ================================================================
# DIFF
# ================================================================

print()
print("=" * 72)
print(" PROPOSED DIFF")
print("=" * 72)

diff = list(
    difflib.unified_diff(
        original.splitlines(keepends=True),
        patched.splitlines(keepends=True),
        fromfile=f"a/{ce_rel}",
        tofile=f"b/{ce_rel}",
    )
)

if not diff:
    print("[ERROR] no diff produced")
    sys.exit(1)

sys.stdout.writelines(diff)

# ================================================================
# DRY RUN
# ================================================================

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Preflight, transformation, and post-patch validation passed.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

# ================================================================
# STAGED SINGLE-FILE REPLACEMENT
# ================================================================
#
# All validation has completed before this point.
# Write to a temporary file in the same directory, fsync it, then replace
# the target with os.replace(). This prevents a partially-written target
# if the write itself is interrupted.

tmp_name = None

try:
    fd, tmp_name = tempfile.mkstemp(
        prefix=".completion_engine.",
        suffix=".tmp",
        dir=str(path.parent),
    )

    with os.fdopen(fd, "w", encoding="utf-8") as tmp:
        tmp.write(patched)
        tmp.flush()
        os.fsync(tmp.fileno())

    os.replace(tmp_name, path)
    tmp_name = None

except Exception as exc:
    if tmp_name is not None:
        try:
            os.unlink(tmp_name)
        except FileNotFoundError:
            pass

    print(f"[WRITE FAILED] {exc}", file=sys.stderr)
    sys.exit(1)

print()
print(f"[PATCH_OK] {ce_rel}")
print("[OK] exactly one source file modified.")
print()
print("Next:")
print("  cargo test -p nexus_runtime_core --lib completion_engine::tests::case_8")
print("  cargo test -p nexus_runtime_core")
PYEOF
