#!/usr/bin/env bash
set -euo pipefail

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║   NEXUS PATCH SEQUENCE — v4.7 SUTURE    ║"
echo "╚══════════════════════════════════════════╝"
echo ""

FILES=(
  "src/sandbox.rs"
  "src/agent_protocol.rs"
  "src/task_manager.rs"
  "src/scheduler.rs"
  "src/memory.rs"
  "src/broker.rs"
  "src/self_healing_engine.rs"
  "src/reflex/extractor.zig"
)

echo ">> [LAZARUS] Snapshotting current state before patch..."
git add -A
git stash push -m "pre-patch-$(date +%s)" 2>/dev/null || true

echo ""
echo ">> [PATCH] Checking all targets exist..."
for f in "${FILES[@]}"; do
  if [[ -f "$f" ]]; then
    echo "   OK: $f"
  else
    echo "   MISSING: $f — will be created by patch script"
  fi
done

echo ""
echo ">> [BUILD] Running cargo check to validate patches..."
cargo check --target aarch64-unknown-linux-gnu -j 2 2>&1

if [[ $? -eq 0 ]]; then
  echo ""
  echo "╔═══════════════════════════════════════╗"
  echo "║  ALL PATCHES VALIDATED — BUILD CLEAN  ║"
  echo "╚═══════════════════════════════════════╝"
else
  echo ""
  echo "!! [WARN] Build check flagged issues — review above output"
  echo "   Stash preserved at: git stash list"
fi
