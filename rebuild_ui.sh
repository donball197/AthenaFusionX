#!/usr/bin/env bash
set -euo pipefail

echo ">> [UI REBUILD] Installing deps and building..."
cd ui
npm install 2>&1 | tail -5
npm run build 2>&1
cd ..

echo ">> [BACKEND] Rebuilding Rust with embedded UI..."
cargo build --target aarch64-unknown-linux-gnu -j 2 2>&1

echo ""
echo "╔═══════════════════════════════════════╗"
echo "║  REBUILD COMPLETE — open localhost:3000 ║"
echo "╚═══════════════════════════════════════╝"
