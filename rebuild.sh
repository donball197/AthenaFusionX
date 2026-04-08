#!/bin/bash
echo "=== NEXUS SINGULARITY REBUILD SEQUENCE ==="
echo "Target: aarch64-unknown-linux-gnu | Cortex-A73"

set -e  # Exit on any error

echo "[1/5] Cleaning old build..."
cargo clean

echo "[2/5] Checking syntax..."
cargo check --target aarch64-unknown-linux-gnu

echo "[3/5] Building optimized release..."
RUSTFLAGS="-Ctarget-cpu=cortex-a73 -Clink-arg=-fuse-ld=lld -Ctarget-feature=+neon,+crc,+lse" \
cargo build --release --target aarch64-unknown-linux-gnu

echo "[4/5] Binary size & info:"
ls -lh target/aarch64-unknown-linux-gnu/release/nexus_singularity
strip target/aarch64-unknown-linux-gnu/release/nexus_singularity 2>/dev/null || true

echo "[5/5] Launching NEXUS..."
./target/aarch64-unknown-linux-gnu/release/nexus_singularity

echo "=== REBUILD COMPLETE ==="
