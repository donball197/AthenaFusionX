#!/bin/bash
set -euo pipefail

echo ">> [NEXUS] Locating nexus_st source files across workspace..."
find . -name "*nexus_st*" -o -name "*.c" -o -name "*.h"

echo ">> [NEXUS] Checking crates/nexus_edge_minion for build.rs..."
if [ -f "crates/nexus_edge_minion/build.rs" ]; then
    cat crates/nexus_edge_minion/build.rs
else
    echo ">> No build.rs in nexus_edge_minion. Creating one to link native library..."
    cat << 'BUILD_RS' > crates/nexus_edge_minion/build.rs
fn main() {
    println!("cargo:rustc-link-search=native=../../lib");
    println!("cargo:rustc-link-lib=dylib=nexus_st");
    println!("cargo:rerun-if-changed=../../lib/libnexus_st.so");
}
BUILD_RS
fi

echo ">> [NEXUS] Compiling native C runtime if source exists..."
if [ -d "lib" ]; then
    cd lib
    if ls *.c 1>/dev/null 2>&1; then
        gcc -O3 -shared -fPIC *.c -o libnexus_st.so
        echo ">> Successfully built libnexus_st.so in lib/"
    elif [ -f "Makefile" ]; then
        make clean && make
    fi
    cd ..
fi

echo ">> [NEXUS] Testing cargo build for nexus_edge_minion..."
cargo build -p nexus_edge_minion
