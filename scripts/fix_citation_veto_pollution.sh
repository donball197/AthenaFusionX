#!/usr/bin/env bash
# fix_citation_veto_pollution.sh
# 1. Diagnoses whether the current strategize.rs still has the raw rfind bug
# 2. Applies the minimal surgical hot-fix
# 3. Verifies the change

set -euo pipefail

STRATEGIZE="crates/runtime_core/src/cortical_loop/strategize.rs"

if [[ ! -f "$STRATEGIZE" ]]; then
  echo "ERROR: $STRATEGIZE not found. Run from the AthenaFusionX root."
  exit 1
fi

echo "=== 1. Diagnosis ==="
if grep -q 'fn citation_veto_pending' "$STRATEGIZE"; then
  echo "Found citation_veto_pending"
else
  echo "ERROR: citation_veto_pending not present"
  exit 1
fi

# Check whether the function still does a raw rfind on the unstripped evidence
if grep -A20 'fn citation_veto_pending' "$STRATEGIZE" | grep -q 'strip_read_bodies_for_markers'; then
  echo "GOOD: already calls strip_read_bodies_for_markers"
  ALREADY_FIXED=1
else
  echo "BUG CONFIRMED: citation_veto_pending does raw rfind / contains without stripping"
  ALREADY_FIXED=0
fi

# Also show the dangerous second pre-flight branch
echo
echo "=== Second (over-broad) citation-pre-flight condition ==="
grep -n -A6 'citation_veto_pending(&session.local_evidence)' "$STRATEGIZE" | head -30

if [[ $ALREADY_FIXED -eq 1 ]]; then
  echo
  echo "Nothing to do – already protected."
  exit 0
fi

echo
echo "=== 2. Applying hot-fix ==="

# Backup
cp "$STRATEGIZE" "${STRATEGIZE}.bak.$(date +%Y%m%d%H%M%S)"

# ------------------------------------------------------------------
# Fix 1: make citation_veto_pending use the strip helper
# ------------------------------------------------------------------
python3 - <<'PY'
import re, sys
path = "crates/runtime_core/src/cortical_loop/strategize.rs"
src = open(path).read()

# Locate the function (tolerant of formatting differences)
pat = re.compile(
    r'(fn citation_veto_pending\(evidence: &str\) -> bool \{\n)'
    r'(\s*)(let Some\(pos\) = evidence\.rfind\("\[CRITIC_VETO\]"\) else \{\n'
    r'\s*return false;\n'
    r'\s*\};)',
    re.MULTILINE
)

def repl(m):
    indent = m.group(2)
    return (
        m.group(1) +
        f"{indent}// Never let markers that appear inside READ_OK bodies trigger a real veto.\n"
        f"{indent}let clean = strip_read_bodies_for_markers(evidence);\n"
        f"{indent}let Some(pos) = clean.rfind(\"[CRITIC_VETO]\") else {{\n"
        f"{indent}    return false;\n"
        f"{indent}}};"
    )

new, n = pat.subn(repl, src, count=1)
if n != 1:
    print("WARNING: could not rewrite citation_veto_pending automatically – manual edit needed", file=sys.stderr)
    sys.exit(1)

# Also fix the tail window so it operates on `clean`
new = new.replace(
    "let tail = &evidence[pos..];",
    "let tail = &clean[pos..];",
    1
)

open(path, "w").write(new)
print("Rewrote citation_veto_pending to use strip_read_bodies_for_markers")
PY

# ------------------------------------------------------------------
# Fix 2: tighten the second (over-broad) pre-flight branch
# ------------------------------------------------------------------
# We only make the second branch require that a write has already happened
# and that a real veto followed it.  This is a pure textual guard.
python3 - <<'PY'
path = "crates/runtime_core/src/cortical_loop/strategize.rs"
src = open(path).read()

old = '''} else if session.contract.is_mutation_task
            && !session.contract.required_artifact_paths.is_empty()
            && citation_veto_pending(&session.local_evidence)
            && !grep_evidence_present(&session.local_evidence)
        {'''

new = '''} else if session.contract.is_mutation_task
            && !session.contract.required_artifact_paths.is_empty()
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
            && artifact_vetoed_after_last_write(&session.local_evidence)
            && citation_veto_pending(&session.local_evidence)
            && !grep_evidence_present(&session.local_evidence)
        {'''

if old not in src:
    print("WARNING: second pre-flight branch not found in exact form – check manually")
else:
    src = src.replace(old, new, 1)
    open(path, "w").write(src)
    print("Tightened second citation-pre-flight branch")
PY

echo
echo "=== 3. Verification ==="
if grep -A15 'fn citation_veto_pending' "$STRATEGIZE" | grep -q 'strip_read_bodies_for_markers'; then
  echo "PASS: citation_veto_pending now strips read bodies"
else
  echo "FAIL: strip still missing"
  exit 1
fi

echo
echo "Done. Rebuild and re-run the same mission:"
echo "  cargo build -p runtime_core   # or your usual nexus.sh path"
echo "  ./nexus.sh"
echo
echo "Expected behaviour after the fix:"
echo "  - first read of strategize.rs no longer triggers citation pre-flight"
echo "  - coverage gap is filled with read_file_chunk"
echo "  - only a real CriticVeto after a WriteOk will force the grep"
