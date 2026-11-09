#!/usr/bin/env bash
# patch-impact-analyzer.sh — migrate the one parse_arg call site the
# main refactor script skipped (impact_analyzer had a divergent
# `flag: &str` signature).
set -euo pipefail

usage() {
    cat <<'USAGE_EOF'
Usage: patch-impact-analyzer.sh [--apply] [--check] [REPO_ROOT]

  (no flags)   Dry-run: print unified diff.
  --apply      Write changes.
  --check      After applying, run cargo check -p impact_analyzer.
USAGE_EOF
    exit 1
}

APPLY=0; CHECK=0; REPO_ROOT=""
for arg in "$@"; do
    case "$arg" in
        --apply) APPLY=1 ;;
        --check) CHECK=1 ;;
        -h|--help) usage ;;
        -*) echo "Unknown flag: $arg" >&2; usage ;;
        *) REPO_ROOT="$arg" ;;
    esac
done
REPO_ROOT="${REPO_ROOT:-$PWD}"
cd "$REPO_ROOT"

TARGET="apps/impact_analyzer/src/main.rs"
if [ ! -f "$TARGET" ]; then
    echo "error: $TARGET not found" >&2; exit 1
fi

python3 - "$APPLY" "$TARGET" <<'PY'
import re, sys, difflib
from pathlib import Path

APPLY = sys.argv[1] == "1"
path  = Path(sys.argv[2])
src   = path.read_text()

# --- 1. Locate and extract fn parse_arg ---
m = re.search(
    r'(?m)^(?:pub\s+)?fn\s+parse_arg\s*\([^)]*\)\s*->\s*Option<String>\s*\{',
    src,
)
if not m:
    print("ERROR: fn parse_arg not found (already migrated?)", file=sys.stderr)
    sys.exit(1)

open_brace = m.end() - 1                    # regex ends with the '{'
depth, i = 0, open_brace
while i < len(src):
    if src[i] == '{': depth += 1
    elif src[i] == '}':
        depth -= 1
        if depth == 0:
            close_brace = i
            break
    i += 1
else:
    print("ERROR: unmatched braces in fn parse_arg", file=sys.stderr)
    sys.exit(1)

fn_start = m.start()
fn_end = close_brace + 1
while fn_end < len(src) and src[fn_end] == '\n':
    fn_end += 1

old_fn = src[fn_start:fn_end]
new = src[:fn_start] + src[fn_end:]

# --- 2. Rewrite call sites ---
# Matches parse_arg(&args, "--X") and parse_arg(&args, "-x"), tolerant of
# whitespace/newlines between tokens.
CALL_RE = re.compile(
    r'parse_arg\(\s*&args\s*,\s*"(-{1,2}[A-Za-z][\w-]*)"\s*\)'
)
calls_before = CALL_RE.findall(new)
new, n = CALL_RE.subn(
    lambda mo: f'nexus_cli_utils::parse_arg(&args, &["{mo.group(1)}"])',
    new,
)

if n == 0:
    print("ERROR: no call sites matched — inspect manually", file=sys.stderr)
    sys.exit(1)

# --- 3. Report + apply/dry-run ---
print()
print("=" * 72)
print("Summary")
print("=" * 72)
print(f"  Removed fn body ({old_fn.count(chr(10))} lines):")
for line in old_fn.rstrip("\n").splitlines():
    print(f"    | {line}")
print()
print(f"  Rewrote {n} call site(s):")
for c in calls_before:
    print(f'    parse_arg(&args, "{c}") -> nexus_cli_utils::parse_arg(&args, &["{c}"])')

print()
print("=" * 72)
if APPLY:
    path.write_text(new)
    print(f"Wrote {path}")
else:
    print("Dry-run — unified diff (pass --apply to write)")
    print("=" * 72)
    sys.stdout.writelines(difflib.unified_diff(
        src.splitlines(keepends=True), new.splitlines(keepends=True),
        fromfile=f"a/{path}", tofile=f"b/{path}"))
PY

if [ "$APPLY" = "1" ] && [ "$CHECK" = "1" ]; then
    echo
    echo "======================================================================"
    echo "Running cargo check -p impact_analyzer"
    echo "======================================================================"
    RUSTFLAGS="-D warnings" cargo check -p impact_analyzer 2>&1 | tail -20
    echo "exit: ${PIPESTATUS[0]}"
fi
