#!/bin/bash
set -e
echo "1. Forging Zig Muscles..."
zig build -Doptimize=ReleaseFast

echo "2. Welding Rust Brain..."
# Linker flag so Rust knows where Zig put the .a file
export RUSTFLAGS="-L ./zig-out/lib"
cargo build --release

echo "3. Testing Sovereign Node..."
./target/release/rebel_compute
