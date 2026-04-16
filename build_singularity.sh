#!/bin/bash
# NEXUS SINGULARITY - Universal Deployer v2.5
set -e 

# Identify absolute root
ROOT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
echo ">> [INIT] Root DNA detected at: $ROOT_DIR"

echo ">> [1/3] Compiling Svelte 5 Frontend..."
cd "$ROOT_DIR/ui" && npm run build

echo ">> [2/3] Compiling Rust Backend..."
cd "$ROOT_DIR" && cargo build --release

echo ">> [3/3] Compiling Zig Substrate (0.15.2 API)..."
cd "$ROOT_DIR/substrate"
# We try to build, but allow a fallback so the server can still launch
zig build -Doptimize=ReleaseSmall || echo "!! [WARNING] Zig substrate failed. Continuing with Cloud-Cortex only."

# --- ROBUST BINARY DETECTION ---
echo ">> [DETECTING] Locating Singularity Binary..."
cd "$ROOT_DIR"

# Aggressively search for the binary
BINARY=$(find "$ROOT_DIR/target" -type f -name "nexus_singularity" | grep release | head -n 1)

if [ -z "$BINARY" ]; then
    echo "!! [ERROR] Rust binary not found in $ROOT_DIR/target."
    exit 1
fi

echo ">> [SUCCESS] Binary detected: $BINARY"
echo ">> [SHIELD] Stripping binary to save RAM..."
strip "$BINARY"

echo ">> [IGNITION] Launching NEXUS Singularity..."
# Use absolute path to execute
"$BINARY"
