#!/bin/bash
# NEXUS_SINGULARITY: Build All Protocol
# ARCH: ARM64 (Chromebook)
# STATUS: Manual Cleanup Protocol Active

echo ">> [INIT] Sovereign Build Sequence initiated..."

# 1. Seating dynamic libraries (Chromebook-native)
echo ">> [SEATING] Mapping Crostini-specific libraries..."
cp lib/chromebook/*.so . 2>/dev/null

# 2. Compile Backend Workspace
echo ">> [KERNEL] Building Rust workspace..."
cargo build --workspace --release

if [ $? -ne 0 ]; then
    echo "!! [FAULT] Backend build failed. Check cargo logs."
    exit 1
fi

# 3. Build UI Assets
echo ">> [UI] Compiling Spatial OS assets..."
cd bins/ide_backend/ui
npm run build
cd ../../..

echo ">> [SUCCESS] Ecosystem compiled."
