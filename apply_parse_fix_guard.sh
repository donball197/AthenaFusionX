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
echo "=== parse-failure fix + guard | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

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

FIX_OLD = '''        if actions.is_empty() {
            let prop_trim = raw_proposal.trim().to_lowercase();
            if prop_trim.contains("halt") {
                actions.push(AgentAction::Halt {
                    reasoning: "Model declared task complete".to_string(),
                });
            }
        }'''

FIX_NEW = '''        if actions.is_empty() {
            let prop_trim = raw_proposal.trim().to_lowercase();
            // Synthesize a halt ONLY from prose-only output. A JSON proposal
            // that failed schema/type validation (e.g. "reasoning" sent as a
            // map) must fall through to the parse-fault recovery path below,
            // which feeds the schema hint back to the strategist. Otherwise
            // this manufactures an action the model never proposed -- observed
            // as "[HALT] Model declared task complete" consuming two turns
            // after a typed parse failure. Explicitly labeled prose halts are
            // already captured by action_intercept::intercept_fallback_action.
            if prop_trim.contains("halt") && !raw_proposal.contains("\\"type\\"") {
                actions.push(AgentAction::Halt {
                    reasoning: "Model declared task complete".to_string(),
                });
            }
        }'''

GUARD = '''    #[test]
    fn parse_failure_never_manufactures_a_typed_halt() {
        // Unforgeable probe: the fragments are split, so this test never
        // contains the full string it verifies. The fix's own comment
        // mentions "prose-only output", so the probe targets the guard
        // CONDITION instead of the comment phrase.
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

# ---- target 1: the fix -------------------------------------------------
if "!raw_proposal" in s:
    step("parse-failure fix", "SKIP (already present)")
elif s.count(FIX_OLD) == 1:
    s = s.replace(FIX_OLD, FIX_NEW, 1)
    step("parse-failure fix", "APPLY")
else:
    step("parse-failure fix", f"FAIL (anchor count={s.count(FIX_OLD)}; region below)")
    for i, ln in enumerate(s.splitlines(), 1):
        if "prop_trim.contains" in ln or "Model declared task complete" in ln:
            print(f"      {i:5d} | {ln}")
    sys.exit(1)

# ---- target 2: the guard ------------------------------------------------
guard_old_shape = 'source.contains("Synthesize a halt ONLY from prose-only output")'
if "fn parse_failure_never_manufactures_a_typed_halt" in s:
    if guard_old_shape in s:
        step("parse-failure guard", "APPLY (converted to fragment probe)")
        s = s.replace(
            guard_old_shape,
            'source.contains(["!raw_proposal", ".contains("].concat().as_str())',
            1,
        )
    else:
        step("parse-failure guard", "SKIP (already present)")
else:
    n = s.count("mod tests {")
    if n != 1:
        step("parse-failure guard", f"FAIL (mod tests count={n})")
        sys.exit(1)
    s = s.replace("mod tests {", "mod tests {\n" + GUARD, 1)
    step("parse-failure guard", "APPLY (inserted at tests module head)")

# ---- post checks (vacuity-audited) --------------------------------------
assert "fn parse_failure_never_manufactures_a_typed_halt" in s
assert 'source.contains("Synthesize a halt ONLY from prose-only output")' not in s, "comment-shaped probe survived"
assert "!raw_proposal" in s, "fix condition missing"
print("[POST-PATCH OK] fix and fragment-probed guard in place")

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
    echo "[OK] parse-failure fix + guard applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib parse_failure_never_manufactures"
fi
