#!/bin/bash
set -euo pipefail

echo ">> [NEXUS] Building native C runtime (libnexus_st) directly..."

# Locate C source files for the native runtime
if [ -d "lib" ]; then
    cd lib
    if [ -f "Makefile" ]; then
        make clean && make
    elif ls *.c 1>/dev/null 2>&1; then
        echo ">> Compiling C sources into libnexus_st.so..."
        gcc -O3 -shared -fPIC *.c -o libnexus_st.so
        echo ">> libnexus_st.so built successfully."
    else
        echo ">> Searching for C source files in workspace..."
        find . -name "*.c" -o -name "*.h"
    fi
    cd ..
else
    echo "!! [WARNING] lib/ directory not found. Checking root for C files..."
    if ls *.c 1>/dev/null 2>&1; then
        gcc -O3 -shared -fPIC *.c -o libnexus_st.so
    fi
fi

echo ">> [NEXUS] Running Phase 2 Golden-Vector RoPE tests..."
cargo test -p nexus_edge_minion --test rope_golden_tests
