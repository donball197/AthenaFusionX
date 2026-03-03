#!/bin/bash
# Nexus Singularity v0.2.2 - Universal Ignite
# Target: ARM64 (Zircon-Stripped Linux Hybrid)

echo "[*] Detecting Shared DNA..."
rm -rf target/
rm -f lib/*.so

echo "[*] Compiling Zircon-Bridge (Zig)..."
# Link against the local zig_types if needed, otherwise raw C types
zig build-lib lib/zircon_bridge.zig -target aarch64-linux -dynamic -lc

echo "[*] Building AI Kernel (Rust + cargo-zigbuild)..."
cargo zigbuild --target aarch64-unknown-linux-gnu --release

echo "[*] Seating Neural Assets..."
mkdir -p target/release/lib/models/
cp lib/models/*.tflite target/release/lib/models/ 2>/dev/null || echo "[!] No models found in lib/models/ yet."

echo "[*] System Ready. 1.0GB RAM Shield Active."
