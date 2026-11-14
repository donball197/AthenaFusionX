#!/usr/bin/env bash
# Surgical fix: make citation_veto_pending immune to marker pollution
# from READ_OK / READ_CHUNK_OK bodies.

set -euo pipefail

FILE="crates/runtime_core/src/cortical_loop/strategize.rs"

if [[ ! -f "$FILE" ]]; then
  echo "ERROR: $FILE not found. Run from AthenaFusionX root."
  exit 1
fi

# Backup
cp "$FILE" "${FILE}.bak.$(date +%Y%m%d%H%M%S)"
echo "Backup created."

python3 - <<'PY'
from pathlib import Path
import re

path = Path("crates/runtime_core/src/cortical_loop/strategize.rs")
src = path.read_text()

# Locate the entire function (tolerant of small formatting differences)
pat = re.compile(
    r'/// True when the most recent \[CRITIC_VETO\].*?\n'
    r'fn citation_veto_pending\(evidence: &str\) -> bool \{\n'
    r'.*?'
    r'(?=\n(?:fn |#\[|/// |pub |impl |$))',
    re.DOTALL
)

new_fn = '''\
/// True when the most recent [CRITIC_VETO] block mentions line numbers,
/// citations, or grep -n. Used by the citation pre-flight gate: after this
/// veto, the strategist's drafts will keep failing without authoritative
/// line numbers, so the cortex forces a grep instead of a fourth draft.
fn citation_veto_pending(evidence: &str) -> bool {
    // Never let markers that appear inside READ_OK / READ_CHUNK_OK bodies
    // (i.e. the source itself) trigger a real veto gate.
    let clean = strip_read_bodies_for_markers(evidence);
    let Some(pos) = clean.rfind("[CRITIC_VETO]") else {
        return false;
    };
    // A veto block is 1-3 KB; take the tail slice from pos to a generous bound.
    let tail = &clean[pos..];
    let window = if tail.len() > 4096 { &tail[..4096] } else { tail };
    window.contains("line")
        || window.contains("citation")
        || window.contains("grep -n")
        || window.contains("unverified")
}
'''

m = pat.search(src)
if not m:
    print("ERROR: could not locate citation_veto_pending function")
    raise SystemExit(1)

# Safety: refuse to overwrite if the strip is already present
if "strip_read_bodies_for_markers(evidence)" in m.group(0):
    print("ALREADY FIXED: citation_veto_pending already calls strip_read_bodies_for_markers")
    raise SystemExit(0)

new_src = src[:m.start()] + new_fn + src[m.end():]
path.write_text(new_src)
print("SUCCESS: citation_veto_pending now strips read bodies before searching for [CRITIC_VETO]")
PY

echo
echo "=== Verification ==="
if grep -A12 'fn citation_veto_pending' "$FILE" | grep -q 'strip_read_bodies_for_markers'; then
  echo "PASS: strip_read_bodies_for_markers is present"
else
  echo "FAIL: strip call missing – inspect the file"
  exit 1
fi

echo
echo "Done. Rebuild and re-run the mission:"
echo "  cargo build -p runtime_core   # or your normal nexus.sh path"
echo "  ./nexus.sh"
