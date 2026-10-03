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
echo "=== repair: HEAVY post-patch assertion | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

SH="apply_verifier_hardening.sh"
if [[ ! -f "$WORKSPACE_ROOT/$SH" ]]; then
    echo "[PREFLIGHT FAILED] missing $SH" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
SH_REL="$SH" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
SH   = os.environ["SH_REL"]

path     = root / SH
original = path.read_text(encoding="utf-8")

# BUG: the post-patch check scans the whole patched file for the literal
# 'Router::select("verify")' -- but HEAVY_NEW's own revert-instruction
# comment contains that exact string, so a SUCCESSFUL replacement trips
# the check on its own comment. Replace it with a precise check against
# HEAVY_OLD (the removed match block): guaranteed gone after the
# replacement, and unforgeable by comments.
CHECK_OLD = r'''if 'Router::select("verify")' in patched[EP]:
    print("[POST-PATCH FAILED] HEAVY: Router verify route still present")
    sys.exit(1)'''

CHECK_NEW = '''if HEAVY_OLD in patched[EP]:
    print("[POST-PATCH FAILED] HEAVY: verify route match block still present")
    sys.exit(1)'''

problems = []

c = original.count(CHECK_OLD)
if c == 0:
    if CHECK_NEW in original:
        problems.append("ALREADY APPLIED: precise HEAVY check is already in place")
    else:
        problems.append("ANCHOR NOT FOUND: the Router::select assertion block is absent")
elif c > 1:
    problems.append(f"ANCHOR x{c} (want exactly 1)")

for label, text in (
    ("HEAVY_OLD definition", "HEAVY_OLD = '''"),
    ("HEAVY_NEW revert comment", "To restore Router routing"),
    ("GUARD test", "fn verifier_claim_check_needs_full_claims_and_grounded_quotes"),
    ("SCHEMA target (on-disk extension)", "SCHEMA"),
):
    if text not in original:
        problems.append(f"INVARIANT VIOLATED: {label} is absent")

if problems:
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    for p in problems:
        print("  -", p)
    sys.exit(1)

print("[PREFLIGHT OK] assertion block matched exactly once")

patched = original.replace(CHECK_OLD, CHECK_NEW, 1)

if patched == original:
    print("[PREFLIGHT FAILED] no textual change produced (internal error)")
    sys.exit(1)

assert patched.count(CHECK_NEW) == 1
assert patched.count(CHECK_OLD) == 0
assert "HEAVY_OLD = '''" in patched

print("[POST-PATCH OK] HEAVY assertion now checks the removed match block, not a file-wide substring")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
sys.stdout.writelines(
    difflib.unified_diff(
        original.splitlines(keepends=True),
        patched.splitlines(keepends=True),
        fromfile=f"a/{SH}",
        tofile=f"b/{SH}",
    )
)

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

path.write_text(patched, encoding="utf-8")
print(f"[PATCH_OK] {SH}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review the diff above."
else
    echo "[OK] assertion repaired."
    echo ">> Re-run:"
    echo "     ./apply_verifier_hardening.sh --dry-run"
    echo "     ./apply_verifier_hardening.sh"
    echo "     cargo test -p nexus_runtime_core"
fi
