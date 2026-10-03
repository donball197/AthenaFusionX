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
echo "=== patch-script repair | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

A="apply_declared_criteria_authority.sh"
B="apply_halt_diagnostics.sh"
for f in "$A" "$B"; do
    if [[ ! -f "$WORKSPACE_ROOT/$f" ]]; then
        echo "[PREFLIGHT FAILED] missing $f" >&2
        exit 1
    fi
done

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
A_REL="$A" B_REL="$B" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
A    = os.environ["A_REL"]
B    = os.environ["B_REL"]

files = {rel: (root / rel, (root / rel).read_text(encoding="utf-8")) for rel in (A, B)}

# Script A: the checks list uses a tuple form. The broken form is
# structurally impossible to satisfy (PROLOGUE_NEW embeds PROLOGUE_OLD).
A_BROKEN = '("PROLOGUE_OLD absent", patched.count(PROLOGUE_OLD), 0),'
A_FIXED  = '("PROLOGUE_NEW present", patched.count(PROLOGUE_NEW), 1),'

# Script B: non-raw triple-quoted anchor vs raw vs compiled-regex form.
B_NON_RAW = "CL_REJECT_OLD = '''"
B_RAW     = "CL_REJECT_OLD = r'''"
B_REGEX   = "CL_PATTERN = re.compile("

problems = []
a_p, b_p = files[A][1], files[B][1]
a_changed = b_changed = False

# --- Script A ---
a_broken = files[A][1].count(A_BROKEN)
a_fixed  = files[A][1].count(A_FIXED)
if a_broken == 1 and a_fixed == 0:
    a_p = files[A][1].replace(A_BROKEN, A_FIXED, 1)
    a_changed = True
    print("[A] will replace over-strict PROLOGUE check with positive check")
elif a_broken == 0 and a_fixed == 1:
    print("[A] already fixed (PROLOGUE_NEW present check)")
else:
    problems.append(
        f"A PROLOGUE check: expected exactly one of {{broken, fixed}}, "
        f"found broken={a_broken} fixed={a_fixed}"
    )

# --- Script B ---
b_non_raw = files[B][1].count(B_NON_RAW)
b_raw     = files[B][1].count(B_RAW)
b_regex   = files[B][1].count(B_REGEX)
if b_non_raw == 1 and b_raw == 0:
    b_p = files[B][1].replace(B_NON_RAW, B_RAW, 1)
    b_changed = True
    print("[B] will convert CL anchor to raw triple-quoted string")
elif b_non_raw == 0 and b_raw == 1:
    print("[B] already raw string")
elif b_non_raw == 0 and b_raw == 0 and b_regex >= 1:
    print("[B] already uses compiled regex (CL_PATTERN); no anchor change needed")
else:
    problems.append(
        f"B CL anchor: found non_raw={b_non_raw} raw={b_raw} regex={b_regex}"
    )

if problems:
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    for p in problems:
        print("  -", p)
    sys.exit(1)

if not a_changed and not b_changed:
    print("[NO-OP] both patch scripts are already in their correct form.")
    print("[NO-OP] Nothing to do.")
    sys.exit(0)

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
for rel, old, new in ((A, files[A][1], a_p), (B, files[B][1], b_p)):
    if old == new:
        print(f"\n--- {rel}: no change ---")
        continue
    sys.stdout.writelines(
        difflib.unified_diff(
            old.splitlines(keepends=True),
            new.splitlines(keepends=True),
            fromfile=f"a/{rel}",
            tofile=f"b/{rel}",
        )
    )

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    sys.exit(0)

if a_changed:
    files[A][0].write_text(a_p, encoding="utf-8")
    print(f"[PATCH_OK] {A}")
if b_changed:
    files[B][0].write_text(b_p, encoding="utf-8")
    print(f"[PATCH_OK] {B}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete."
else
    echo "[OK] patch-script repair complete."
fi
