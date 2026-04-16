#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
set -e

echo "[*] Detecting Shared DNA..."

# Mandatory purge of cross-platform artifacts before compilation
echo "[*] Purging residual target binaries and specific .so libraries..."
rm -rf target/
rm -f *.so

mkdir -p target/release

echo "[*] Building AI Kernel (Native Rust for Host Libraries)..."
cargo build --release

echo "[*] System Ready. Igniting..."
$(find target -name nexus_singularity -type f | head -n 1)
