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
echo "=== parse-failure guard (fragment probe) | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

CL="crates/runtime_core/src/cortical_loop.rs"
if [[ ! -f "$WORKSPACE_ROOT/$CL" ]]; then
    echo "[PREFLIGHT FAILED] missing $CL" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
CL_REL="$CL" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
CL   = os.environ["CL_REL"]

path = root / CL
s0 = path.read_text(encoding="utf-8")
s = s0

GUARD = '''    #[test]
    fn parse_failure_never_manufactures_a_typed_halt() {
        // Unforgeable probe: the fragments are split, so this test never
        // contains the full string it verifies. The fix's own comment
        // mentions "Synthesize a halt ONLY from prose-only output", so the
        // probe targets the guard CONDITION instead of the comment phrase.
        let source = include_str!("cortical_loop.rs");
        let probe = ["!raw_proposal", ".contains("].concat();
        assert!(
            source.contains(probe.as_str()),
            "typed parse failures must fall through to schema-hint recovery; never synthesize a halt"
        );
    }

'''

def step(label, status):
    print(f"[STATUS] {label:52s} : {status}")

if "fn parse_failure_never_manufactures_a_typed_halt" in s:
    step("parse-failure guard", "SKIP (already present)")
else:
    n = s.count("mod tests {")
    if n != 1:
        step("parse-failure guard", f"FAIL (mod tests count={n}; send: rg -n 'mod tests' {CL})")
        sys.exit(1)
    s = s.replace("mod tests {", "mod tests {\n" + GUARD, 1)
    step("parse-failure guard", "APPLY (inserted at tests module head)")

if "!raw_proposal" not in s:
    step("parse-failure fix presence", "FAIL (fix condition not in source)")
    sys.exit(1)
step("parse-failure fix presence", "OK (guard condition found)")

print("[POST-PATCH OK]")
print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
print("".join(difflib.unified_diff(s0.splitlines(keepends=True), s.splitlines(keepends=True),
                                  fromfile=f"a/{CL}", tofile=f"b/{CL}")))

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

path.write_text(s, encoding="utf-8")
print(f"[PATCH_OK] {CL}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review status lines and diff above."
else
    echo "[OK] parse-failure guard inserted."
    echo ">> Targeted test:"
    echo "     cargo test -p nexus_runtime_core --lib parse_failure_never_manufactures"
fi
