#!/bin/bash
echo ">> [REBUILD] Compiling Zig Muscle Substrate..."
zig build-lib src/kernel/muscle.zig -O ReleaseFast -femit-bin=libmuscle.a -lc

if [ $? -eq 0 ]; then
    echo ">> [SUCCESS] libmuscle.a generated."
    echo ">> [REBUILD] Compiling Rust DNA..."
    cargo build -j1
else
    echo "!! [FAILURE] Zig compilation failed."
    exit 1
fi
