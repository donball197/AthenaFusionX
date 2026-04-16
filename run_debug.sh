#!/usr/bin/env bash
# run_debug.sh — Runs NEXUS with full raw response logging
# Usage: ./run_debug.sh
# Then type: task "QueryMemory for 'SQLite WAL mode'"
# You will see the raw model output each turn so you can diagnose JSON failures.

export NEXUS_DEBUG=1
export GEMINI_API_KEY="${GEMINI_API_KEY:-$(grep GEMINI_API_KEY .env | cut -d= -f2)}"

echo ">> [DEBUG MODE] Raw model responses will be shown each turn."
echo ">> Building..."
cargo build --target aarch64-unknown-linux-gnu -j 2 2>&1

if [[ $? -eq 0 ]]; then
    echo ">> Running NEXUS with debug output..."
    NEXUS_DEBUG=1 ./target/aarch64-unknown-linux-gnu/debug/nexus_singularity
else
    echo "!! Build failed — check errors above"
fi
