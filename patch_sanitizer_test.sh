#!/usr/bin/env bash
# patch_sanitizer_test.sh — add behavioral tests for
# sanitize_ungrounded_citation_veto, the critic-veto sanitizer wired into
# cortical_loop/critic.rs.
#
# Appends a new #[cfg(test)] mod citation_sanitizer_tests at the end of
# critic.rs. Idempotent: refuses to double-apply.

set -euo pipefail

ROOT="$(pwd)"
CRITIC="$ROOT/crates/runtime_core/src/cortical_loop/critic.rs"
APPLY=0
FULL=0

for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --full)  FULL=1 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

[[ -f "$CRITIC" ]] || { echo "ERROR: $CRITIC not found" >&2; exit 1; }

if grep -q 'mod citation_sanitizer_tests' "$CRITIC"; then
  echo "ALREADY APPLIED: citation_sanitizer_tests module exists"
  exit 0
fi

STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$CRITIC.bak.$STAMP"
cp "$CRITIC" "$BACKUP"
echo "backup: $BACKUP"

TEST_BLOCK='
#[cfg(test)]
mod citation_sanitizer_tests {
    use super::*;

    #[test]
    fn sanitizer_strips_invented_line_ranges() {
        // Exact critique from turn 14 of the failed marker_summary trace.
        // Every line number is invented — real source has is_marker_line
        // at 95, cmd_ok_binds_command at 111, read_path_satisfied at 140.
        let bad = "the proposed halt is premature because marker_summary.md \
                   cites incorrect line ranges. Source shows `is_marker_line` \
                   at lines 80-90, `cmd_ok_binds_command` at lines 93-114, \
                   and `read_path_satisfied` at lines 117-142, not 95-105, \
                   111-132, and 140-165.";

        let clean = sanitize_ungrounded_citation_veto(bad, "");

        for forbidden in ["80-90", "93-114", "117-142", "95-105", "111-132", "140-165"] {
            assert!(
                !clean.contains(forbidden),
                "invented range {:?} survived sanitization: {}",
                forbidden,
                clean
            );
        }

        assert!(
            clean.contains("premature") || clean.contains("incorrect"),
            "sanitizer stripped the entire critique, leaving no guidance: {}",
            clean
        );
    }

    #[test]
    fn sanitizer_passes_through_critiques_without_line_numbers() {
        let clean_input = "the artifact does not cite the specific functions \
                           the objective requires.";
        let output = sanitize_ungrounded_citation_veto(clean_input, "");

        assert!(
            output.contains("does not cite the specific functions"),
            "sanitizer lost semantic content: {}",
            output
        );
        assert!(
            !output.contains("lines"),
            "sanitizer introduced \"lines\" where input had none: {}",
            output
        );
    }
}
'

python3 - "$CRITIC" "$TEST_BLOCK" <<'PY'
import sys
path, test_block = sys.argv[1], sys.argv[2]
src = open(path, encoding='utf-8').read()
if 'mod citation_sanitizer_tests' in src:
    sys.exit(0)
if not src.rstrip().endswith('}'):
    sys.stderr.write("ERROR: critic.rs does not end with \"}\" — structure changed\n")
    sys.exit(3)
open(path, 'w', encoding='utf-8').write(src.rstrip() + '\n' + test_block)
PY

echo
echo "===== appended test module ====="
tail -55 "$CRITIC"
echo "================================"
echo

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

rollback() {
  echo >&2
  echo "!! verification failed; rolling back" >&2
  cp "$BACKUP" "$CRITIC"
  exit 1
}
trap 'rollback' ERR

echo ">> cargo check -p nexus_runtime_core ..."
cargo check -p nexus_runtime_core || rollback

echo ">> cargo test -p nexus_runtime_core --lib citation_sanitizer_tests ..."
cargo test -p nexus_runtime_core --lib citation_sanitizer_tests || rollback

if [[ "$FULL" -eq 1 ]]; then
  echo ">> cargo test -p nexus_runtime_core --lib ..."
  cargo test -p nexus_runtime_core --lib || rollback
fi

trap - ERR
echo
echo "OK: sanitizer tests applied and verified."
