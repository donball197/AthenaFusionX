#!/usr/bin/env bash
set -euo pipefail

echo "╔═══════════════════════════════════════╗"
echo "║  NEXUS FULL REBUILD                   ║"
echo "╚═══════════════════════════════════════╝"

# Kill any existing instance
fuser -k 3000/tcp 2>/dev/null || true

echo ""
echo ">> [1/3] cargo check..."
cargo check --target aarch64-unknown-linux-gnu -j 2 2>&1
echo ">> Rust: CLEAN"

echo ""
echo ">> [2/3] Building Svelte UI..."
cd ui && npm run build 2>&1 | tail -8 && cd ..
echo ">> UI: BUILT"

echo ""
echo ">> [3/3] Building Rust binary..."
cargo build --target aarch64-unknown-linux-gnu -j 2 2>&1 | tail -5
echo ">> Binary: READY"

echo ""
echo "╔═══════════════════════════════════════════════╗"
echo "║  LAUNCHING NEXUS                              ║"
echo "║  Browser: http://localhost:3000               ║"
echo "║  CLI:     type directives below               ║"
echo "║  IDE:     click files or type path to open    ║"
echo "║  Save:    Ctrl+S or click 💾 SAVE button      ║"
echo "╚═══════════════════════════════════════════════╝"
echo ""

./target/aarch64-unknown-linux-gnu/debug/nexus_singularity
