#!/bin/bash
# Fail if cortical_loop.rs exceeds MAX lines. Lower MAX after every extraction.
MAX=${MAX:-2450}
n=$(wc -l < crates/runtime_core/src/cortical_loop.rs)
[ "$n" -gt "$MAX" ] && { echo "cortical_loop.rs is $n lines (max $MAX)"; exit 1; }
echo "ok: $n / $MAX"
