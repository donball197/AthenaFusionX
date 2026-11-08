#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
F=crates/nexus_substrate/examples/diagnose_memory.rs
[ -f "$F.prepatch" ] || cp "$F" "$F.prepatch"
python3 - "$F" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
old = 'palace.semantic_search(&embedding, 3).await.context("production semantic_search failed")?;'
new = '''palace
            .semantic_search_taint_scoped(
                &embedding,
                3,
                nexus_substrate::taint::FlowPolicy::INTERNAL_SINK,
            )
            .await
            .context("production semantic_search failed")?;'''
if s.count(old) != 1:
    sys.exit(f"[FAIL] anchor found {s.count(old)} times; paste lines 58-66 of {p}")
open(p, "w").write(s.replace(old, new))
print("[ok] example patched")
PY
CARGO_TERM_COLOR=never cargo +nightly check --workspace --all-targets 2>&1 | grep -E '^error' -A8 | head -30 || true
echo "--- done: empty above means clean ---"
