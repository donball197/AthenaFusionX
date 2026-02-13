#!/bin/bash
# Project Nexus v0.2.2 - THE FAST TRACK

# Clean old slow fragments
rm -rf lib/*.a
mkdir -p lib

echo ">> [1/3] FORGING ZIG MUSCLES (RELEASE-FAST)..."
zig build-lib zig_muscle/resonator.zig \
    -target aarch64-linux \
    -O ReleaseFast \
    -fPIC \
    -femit-bin=lib/libresonator.a

echo ">> [2/3] IGNITING RUST SKELETON (OPT-LEVEL 3)..."
# Link the Zig muscle directly into the Rust binary
export RUSTFLAGS="-L ./lib -l resonator -C opt-level=3 -C target-cpu=native"

# Check if we are on the Duet (Foundation) or Motorola (Field)
if command -v gh &> /dev/null; then
    echo ">> [FOUNDATION] SYNCING TO GITHUB..."
    cargo build --release
    gh repo sync
else
    echo ">> [FIELD] MOTOROLA G STRESS TEST..."
    cargo build --release
    ./target/release/nexus_singularity
fi

echo ">> [3/3] SINGULARITY ACHIEVED."
