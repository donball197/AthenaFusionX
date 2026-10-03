#!/usr/bin/env bash
set -euo pipefail

WORKSPACE_ROOT="$(realpath -m "${WORKSPACE_ROOT:-$(pwd)}")"
echo "=== hygiene regions survey (read-only) | root: $WORKSPACE_ROOT ==="

if [[ ! -d "$WORKSPACE_ROOT/crates/runtime_core/src" ]]; then
    echo "[SURVEY FAILED] missing crates/runtime_core/src" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" python3 - <<'PYEOF'
import os, pathlib

root = pathlib.Path(os.environ["NEXUS_ROOT"])
src = root / "crates/runtime_core/src"
rs_files = sorted(src.rglob("*.rs"))

# (label, probe, span, max_hits)
probes = [
    # --- H-1: [Contradicts] tag misattribution ---
    ("H1 Contradicts literal",        '"[Contradicts]"',                    25, 3),
    ("H1 Supports literal",           '"[Supports]"',                       15, 3),
    ("H1 SUCCESS PATTERN",            "SUCCESS PATTERN",                    25, 3),
    ("H1 LESSON failure-on-success",  "Action triggered failure",           15, 2),
    ("H1 relation classification",    "EdgeRelation::Contradicts",          12, 4),

    # --- H-2: double evidence append ---
    ("H2 collect_evidence",           "fn collect_evidence",                35, 2),
    ("H2 PHYSICAL SESSION EVIDENCE",  "[PHYSICAL SESSION EVIDENCE]",        20, 3),
    ("H2 EVIDENCE ACQUIRED",          "[EVIDENCE ACQUIRED]",                15, 3),
    ("H2 local_evidence.push",        "local_evidence.push",                8,  8),

    # --- H-3: goal lifecycle ---
    ("H3 intent pulse",               "Maintaining intent vector",          20, 2),
    ("H3 DAG COMPLETE",               "DAG COMPLETE",                       18, 3),
    ("H3 goal registry",              "Registered goal",                    18, 2),
    ("H3 active goals",               "active goal",                        12, 4),
]

def dump(lines, idx, span):
    lo, hi = max(0, idx - span), min(len(lines), idx + span)
    for i in range(lo, hi):
        print(f"      {i + 1:5d} | {lines[i]}")

for label, probe, span, max_hits in probes:
    hits = []
    for f in rs_files:
        try:
            text = f.read_text(encoding="utf-8")
        except Exception:
            continue
        if probe in text:
            lines = text.splitlines()
            for i, ln in enumerate(lines):
                if probe in ln:
                    hits.append((f, lines, i))
                    if len(hits) >= max_hits:
                        break
    if not hits:
        print(f"[STATUS] {label:34s} : ABSENT")
        continue
    total = sum(sum(1 for ln in L if probe in ln) for _, L, _ in hits)
    f, lines, idx = hits[0]
    print(f"[STATUS] {label:34s} : FOUND x{total}  (first: {f.relative_to(root)}:{idx + 1})")
    dump(lines, idx, span)
    for f2, _, i2 in hits[1:]:
        print(f"      [also: {f2.relative_to(root)}:{i2 + 1}]")
    print()
PYEOF

echo
echo "[SURVEY COMPLETE] zero files written. Paste the output back for apply_hygiene_batch.sh."
