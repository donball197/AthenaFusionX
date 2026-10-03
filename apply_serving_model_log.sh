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
echo "=== serving-model attempt log | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

EP="crates/runtime_core/src/epistemic_engine.rs"
if [[ ! -f "$WORKSPACE_ROOT/$EP" ]]; then
    echo "[PREFLIGHT FAILED] missing $EP" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
EP_REL="$EP" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
EP   = os.environ["EP_REL"]

files = {EP: (root / EP, (root / EP).read_text(encoding="utf-8"))}
patched = {EP: files[EP][1]}

ATTEMPT_OLD = '''            loop {
                let (url, req_payload) = match resolved.provider_name {'''

ATTEMPT_NEW = '''            loop {
                eprintln!("[MODEL TRACE] -> attempt model={} provider={} retry={}", model_endpoint, resolved.provider_name, retries);
                let (url, req_payload) = match resolved.provider_name {'''

TEST_ANCHOR = '''    #[tokio::test]
    async fn compound_read_plus_search_code_pushes_search_first() {'''

TEST_INSERT = '''    #[test]
    fn safe_api_call_logs_every_attempt() {
        let source = include_str!("epistemic_engine.rs");
        assert!(
            source.contains("[MODEL TRACE] -> attempt model="),
            "each cascade attempt must be logged for serving-model attribution"
        );
    }

''' + TEST_ANCHOR

targets = [
    ("ATTEMPT", EP, "safe_api_call: per-attempt serving-model log",
     ATTEMPT_OLD, ATTEMPT_NEW,
     '"[MODEL TRACE] -> attempt model=', '"[MODEL TRACE] -> attempt model=',
     "fn safe_api_call", 20),
    ("GUARD",   EP, "epistemic_engine: attempt-log guard test",
     TEST_ANCHOR, TEST_INSERT,
     "fn safe_api_call_logs_every_attempt",
     "fn safe_api_call_logs_every_attempt",
     "compound_read_plus_search_code_pushes_search_first", 5),
]

def dump_region(rel, probe, span):
    lines = files[rel][1].splitlines()
    idx = next((i for i, ln in enumerate(lines) if probe in ln), None)
    if idx is None:
        print(f"    [REGION DUMP FAILED] probe not found in {rel}: {probe!r}")
        return
    lo, hi = max(0, idx - span), min(len(lines), idx + span)
    print(f"    [REGION] {rel} around line {idx + 1} (probe: {probe!r}):")
    for i in range(lo, hi):
        print(f"      {i + 1:5d} | {lines[i]}")

plan = []
failures = []

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    text = patched[rel]
    old_count = text.count(old)
    if already in text:
        status = "SKIP (already present)"
        plan.append((key, rel, None))
    elif old_count == 1:
        status = "APPLY"
        plan.append((key, rel, (old, new)))
    elif old_count > 1:
        status = f"FAIL (anchor x{old_count}, want exactly 1)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    else:
        status = "FAIL (anchor not found)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    print(f"[STATUS] {key:7s} {label:50s} : {status}")

for rel, label, text in (
    (EP, "cascade entry log", '[MODEL TRACE] safe_api_call models='),
    (EP, "provider resolution", "resolve_provider(model_endpoint)"),
    (EP, "empty-cascade error", "No models configured"),
):
    if text not in files[rel][1]:
        print(f"[INVARIANT VIOLATED] {label} is absent in {rel}")
        failures.append(("INVARIANT", rel, label, text, 5))

if failures:
    print()
    print("[DIAGNOSTIC] region dumps for failed targets:")
    for key, rel, label, region_probe, span in failures:
        print(f"  -- {key}: {label}")
        dump_region(rel, region_probe, span)
    print()
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    sys.exit(1)

changed = []
for key, rel, edit in plan:
    if edit is None:
        continue
    old, new = edit
    patched[rel] = patched[rel].replace(old, new, 1)
    changed.append((key, rel))

if not changed:
    print()
    print("[NO-OP] every target is already in its final form. Nothing to write.")
    sys.exit(0)

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    if final_probe not in patched[rel]:
        print(f"[POST-PATCH FAILED] {label}: final probe absent: {final_probe!r}")
        sys.exit(1)

print()
print(f"[POST-PATCH OK] {len(changed)} target(s) transformed: {', '.join(k for k, _ in changed)}")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
sys.stdout.writelines(
    difflib.unified_diff(
        files[EP][1].splitlines(keepends=True),
        patched[EP].splitlines(keepends=True),
        fromfile=f"a/{EP}",
        tofile=f"b/{EP}",
    )
)

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

files[EP][0].write_text(patched[EP], encoding="utf-8")
print(f"[PATCH_OK] {EP}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review the status lines and diff above."
else
    echo "[OK] attempt logging applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib epistemic_engine"
fi
