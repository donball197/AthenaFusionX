#!/bin/bash
set -e

echo "[*] Detecting Shared DNA..."
mkdir -p target/release

echo "[*] Building AI Kernel (Native Rust for Host Libraries)..."
cargo build --release

echo "[*] Seating Neural Assets..."
mkdir -p target/release/lib/models/
if [ -d "lib/models" ]; then
    cp lib/models/*.tflite target/release/lib/models/ 2>/dev/null || true
fi

echo "[*] System Ready. Igniting..."
$(find target -name nexus_singularity -type f | head -n 1)
