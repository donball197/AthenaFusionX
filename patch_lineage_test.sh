#!/usr/bin/env bash
# patch_lineage_test.sh — fix \\n → \n in cortical_loop lineage-hash test
#
# The function
#   cortical_loop::inherit_evidence_tests::lineage_hash_matches_filtered_payload_verified_by_consumer
# uses literal backslash-n (\\n) in its ancestor and envelope strings instead
# of real newlines (\n). Consequence: inherit_physical_evidence sees a single
# line, does not filter, and verify_lineage_provenance cannot parse a
# single-line envelope — so the second assertion fails.
#
# Usage
#   ./patch_lineage_test.sh                    # dry-run, prints diff
#   ./patch_lineage_test.sh --apply            # apply + targeted test
#   ./patch_lineage_test.sh --apply --full     # also full --lib suite
#   ./patch_lineage_test.sh --help

set -euo pipefail

ROOT="$(pwd)"
SRC="$ROOT/crates/runtime_core/src/cortical_loop.rs"
APPLY=0
FULL=0

for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --full)  FULL=1 ;;
    -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

[[ -f "$SRC" ]] || { echo "ERROR: $SRC not found (run from repo root)" >&2; exit 1; }

STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$SRC.bak.$STAMP"
cp "$SRC" "$BACKUP"
echo "backup: $BACKUP"

PATCHED="$(mktemp)"
trap 'rm -f "$PATCHED"' EXIT

python3 - "$SRC" "$PATCHED" <<'PY'
import sys

src_path, out_path = sys.argv[1], sys.argv[2]
src = open(src_path, encoding='utf-8').read()

FN_NAME = "fn lineage_hash_matches_filtered_payload_verified_by_consumer"

start = src.find(FN_NAME)
if start < 0:
    sys.stderr.write(f"ERROR: {FN_NAME} not found in {src_path}\n")
    sys.exit(3)

brace_open = src.find('{', start)
if brace_open < 0:
    sys.stderr.write("ERROR: no opening brace for target function\n")
    sys.exit(3)

def find_body_end(s, open_idx):
    i = open_idx
    depth = 0
    n = len(s)
    in_str = in_char = in_line = False
    block = 0
    while i < n:
        c = s[i]; nxt = s[i+1] if i+1 < n else ''
        if in_line:
            if c == '\n': in_line = False
            i += 1; continue
        if block:
            if c == '*' and nxt == '/': block -= 1; i += 2; continue
            if c == '/' and nxt == '*': block += 1; i += 2; continue
            i += 1; continue
        if in_str:
            if c == '\\': i += 2; continue
            if c == '"': in_str = False
            i += 1; continue
        if in_char:
            if c == '\\': i += 2; continue
            if c == "'": in_char = False
            i += 1; continue
        if c == '/' and nxt == '/': in_line = True; i += 2; continue
        if c == '/' and nxt == '*': block = 1; i += 2; continue
        if c == '"': in_str = True; i += 1; continue
        if c == "'": in_char = True; i += 1; continue
        if c == '{': depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0: return i + 1
        i += 1
    return None

body_end = find_body_end(src, brace_open)
if body_end is None:
    sys.stderr.write("ERROR: unbalanced braces in target function\n")
    sys.exit(3)

body = src[brace_open:body_end]

preconditions = [
    ('"unfiltered ancestor hash must not validate a filtered payload"', 1),
    ('"filtered payload and declared SHA-256 must agree"', 1),
    ('let ancestor = ', 1),
    ('let payload = ', 1),
    ('let old_envelope = ', 1),
    ('let envelope = ', 1),
    ('verify_lineage_provenance', 2),
]
for needle, expected in preconditions:
    actual = body.count(needle)
    if actual != expected:
        sys.stderr.write(
            f"ERROR: precondition failed: {needle!r} appears {actual}x "
            f"(expected {expected}x). Refusing to auto-patch.\n"
        )
        sys.exit(3)

pre_count = body.count(r'\\n')
if pre_count == 0:
    sys.stderr.write(
        "ERROR: no \\\\n found inside target function. "
        "Either the file was already fixed, or the function has changed shape.\n"
    )
    sys.exit(3)

new_body = body.replace(r'\\n', r'\n')

if r'\\n' in new_body:
    sys.stderr.write("ERROR: \\\\n still present after replacement\n")
    sys.exit(3)

post_count = new_body.count(r'\n')
if post_count != pre_count:
    sys.stderr.write(
        f"ERROR: expected {pre_count} \\n after replacement, found {post_count}\n"
    )
    sys.exit(3)

sys.stderr.write(f"info: replaced {pre_count} occurrences of \\\\n with \\n\n")

new_src = src[:brace_open] + new_body + src[body_end:]
open(out_path, 'w', encoding='utf-8').write(new_src)
PY

echo
echo "===== diff ($BACKUP -> patched) ====="
diff -u "$BACKUP" "$PATCHED" || true
echo "===================================="
echo

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

install -m 0644 "$PATCHED" "$SRC"
echo "installed patched cortical_loop.rs"

rollback() {
  echo >&2
  echo "!! verification failed; rolling back" >&2
  cp "$BACKUP" "$SRC"
  echo "restored $SRC from $BACKUP" >&2
  exit 1
}
trap 'rollback' ERR

echo ">> cargo check -p nexus_runtime_core ..."
cargo check -p nexus_runtime_core || rollback

echo ">> cargo test -p nexus_runtime_core --lib cortical_loop::inherit_evidence_tests ..."
cargo test -p nexus_runtime_core --lib cortical_loop::inherit_evidence_tests || rollback

if [[ "$FULL" -eq 1 ]]; then
  echo ">> cargo test -p nexus_runtime_core --lib ..."
  cargo test -p nexus_runtime_core --lib || rollback
fi

trap - ERR
echo
echo "OK: patch applied and verified."
