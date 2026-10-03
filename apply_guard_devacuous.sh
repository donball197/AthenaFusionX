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
echo "=== guard de-vacuous (fragment probes) | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

EP="crates/runtime_core/src/epistemic_engine.rs"
CL="crates/runtime_core/src/cortical_loop.rs"
for f in "$EP" "$CL"; do
    if [[ ! -f "$WORKSPACE_ROOT/$f" ]]; then
        echo "[PREFLIGHT FAILED] missing $f" >&2
        exit 1
    fi
done

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
EP_REL="$EP" CL_REL="$CL" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
EP   = os.environ["EP_REL"]
CL   = os.environ["CL_REL"]

paths = {EP: root / EP, CL: root / CL}
orig  = {k: v.read_text(encoding="utf-8") for k, v in paths.items()}
s     = orig[EP]
s2    = orig[CL]

HELPER_FN = '''    /// Source guard with an unforgeable probe: assembled at runtime from
    /// fragments, so this test file never contains the full string it looks
    /// for. A plain source.contains("...") guard is vacuous -- the probe also
    /// lives in the test's own source and passes unconditionally.
    fn source_has(pieces: &[&str]) -> bool {
        let source = include_str!("epistemic_engine.rs");
        source.contains(pieces.concat().as_str())
    }

'''

ANCHOR = "    use super::EpistemicEngine;\n    use reqwest::Client;"

CONV = [
    ('source.contains("action.chars().take(6000)")',
     'source_has(&["action.chars().take(", "6000)"])'),
    ('source.contains("quote the exact source lines from EVIDENCE")',
     'source_has(&["quote the exact source lines from ", "EVIDENCE"])'),
    ('source.contains("eprintln!(\\"[VERIFIER] model cascade")',
     'source_has(&["eprintln!(\\"[VERIFIER]", " model cascade"])'),
    ('source.contains("CRITICAL RULE 6 [CLAIM CHECK]")',
     'source_has(&["CRITICAL RULE 6", " [CLAIM CHECK]"])'),
    ('source.contains("confidence_modifier of -0.30 or lower")',
     'source_has(&["confidence_modifier of -0.30", " or lower"])'),
    ('source.contains("apply it mentally to the case in the ORIGINAL DIRECTIVE")',
     'source_has(&["apply it mentally to the case", " in the ORIGINAL DIRECTIVE"])'),
    ('source.contains("\\"[CRITIC_UNPARSEABLE]\\"")',
     'source_has(&["[CRITIC", "_UNPARSEABLE]"])'),
    ('source.contains("[MODEL TRACE] -> attempt model=")',
     'source_has(&["[MODEL TRACE] -> attempt", " model="])'),
]

def step(label, status):
    print(f"[STATUS] {label:56s} : {status}")

if "fn source_has(" in s:
    step("source_has helper (epistemic_engine)", "SKIP (already present)")
elif s.count(ANCHOR) == 1:
    s = s.replace(ANCHOR, ANCHOR + "\n" + HELPER_FN, 1)
    step("source_has helper (epistemic_engine)", "APPLY")
else:
    step("source_has helper (epistemic_engine)", "FAIL (anchor not unique)")
    sys.exit(1)

converted = 0
for old, new in CONV:
    n = s.count(old)
    if n == 0:
        step(f"convert probe {old[16:52]!r}", "SKIP (absent)")
        continue
    s = s.replace(old, new)
    converted += n
    step(f"convert probe {old[16:52]!r}", f"APPLY x{n}")
step("converted probes (epistemic_engine)", f"{converted}")

CL_OLD = 'source.contains("Synthesize a halt ONLY from prose-only output")'
CL_NEW = 'source.contains(["Synthesize a halt ONLY from prose-only ", "output"].concat().as_str())'
if "fn parse_failure_never_manufactures_a_typed_halt" not in s2:
    # Nothing to convert. NOTE: the fix's own COMMENT contains the phrase
    # "Synthesize a halt ONLY from prose-only output", so phrase-presence
    # cannot decide this -- the guard function name is the discriminator.
    step("cortical_loop guard fragment probe", "SKIP (no guard to convert)")
elif CL_NEW in s2:
    step("cortical_loop guard fragment probe", "SKIP (already converted)")
elif s2.count(CL_OLD) == 1:
    s2 = s2.replace(CL_OLD, CL_NEW, 1)
    step("cortical_loop guard fragment probe", "APPLY")
else:
    # Guard exists in some other probe form (e.g. inserted fragment-guarded):
    # conversion is moot. This step is best-effort and never blocks the batch.
    step("cortical_loop guard fragment probe", "SKIP (guard present in fragment form)")

# ---------------- post checks (vacuity-audited) -------------------------
assert s.count("fn source_has(") == 1
assert "source.contains(\"action.chars().take(6000)\")" not in s, "a vacuous probe survived"
assert s.count("source_has(&[") >= 5, "conversion did not take effect"
# NOTE: exactly 5 conversion targets are defined in CONV; do not hard-code a
# count that contradicts the plan. Behavior is proven by cargo test, not here.
assert "source.contains(\"Synthesize a halt ONLY from prose-only output\")" not in s2
print("[POST-PATCH OK] all guard probes are fragment-assembled (unforgeable by their own test text)")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
for key, before, after in ((EP, orig[EP], s), (CL, orig[CL], s2)):
    if before != after:
        print("".join(difflib.unified_diff(before.splitlines(keepends=True),
                                          after.splitlines(keepends=True),
                                          fromfile=f"a/{key}", tofile=f"b/{key}")))

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

paths[EP].write_text(s, encoding="utf-8")
print(f"[PATCH_OK] {EP}")
paths[CL].write_text(s2, encoding="utf-8")
print(f"[PATCH_OK] {CL}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review status lines and diff above."
else
    echo "[OK] guards de-vacuousd."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib"
fi
