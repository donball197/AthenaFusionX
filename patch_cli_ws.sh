#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
F=bins/cli/src/main.rs
python3 - "$F" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
old = 'let ws_url = "ws://127.0.0.1:3000/ws/telemetry";'
new = 'let ws_url: &\'static str = Box::leak(format!("ws://127.0.0.1:3000/ws/telemetry?token={}", token.trim()).into_boxed_str());'
if "telemetry?token=" in s:
    print("[skip] already patched"); sys.exit(0)
if s.count(old) != 1:
    sys.exit(f"[FAIL] anchor count {s.count(old)}; paste lines 10-25 of {p}")
open(p, "w").write(s.replace(old, new))
print("[ok] cli patched")
PY
CARGO_TERM_COLOR=never cargo +nightly check -p nexus_cli 2>&1 | grep -E '^error' -A8 | head -20 || true
echo "--- done: empty above means clean ---"
