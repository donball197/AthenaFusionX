#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo "[*] Cleaning fragments..."
rm -rf target/
mkdir -p vault missions claims archives .lazarus/snapshots

echo "[*] Building Visor UI (Svelte)..."
cd ui
# Ensure we have dependencies
npm install --quiet
# Force a clean build
rm -rf dist
npm run build
cd ..

echo "[*] Compiling Kernel (Rust)..."
cargo build --release

echo "[*] NEXUS SINGULARITY SEATED."
