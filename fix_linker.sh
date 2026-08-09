#!/bin/bash
# fix_linker.sh - Project Nexus Singularity v0.2.2

echo ">> [NEXUS] Forging build.rs to link the Zig FFI library..."

cat << 'EOF' > crates/nexus_edge_minion/build.rs
use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let lib_dir = PathBuf::from(manifest_dir).join("../../lib");

    // Tell Cargo where to search for libraries
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    
    // Tell Cargo to link against libnexus_loader.so
    println!("cargo:rustc-link-lib=dylib=nexus_loader");
    
    // Re-run this script if the Zig library changes
    println!("cargo:rerun-if-changed=../../lib/libnexus_loader.so");
}
EOF

echo ">> [NEXUS] build.rs injected. Recompiling the substrate..."
cargo build -p nexus_edge_minion

echo ">> [NEXUS] Ready to ignite the engine:"
echo "cargo run -p nexus_edge_minion"
