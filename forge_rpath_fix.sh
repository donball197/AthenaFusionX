#!/bin/bash
# forge_rpath_fix.sh - one-time fix: bake lib/ into nexus_edge_minion's rpath
# so LD_LIBRARY_PATH is never needed again to run the binary.
#
# Run from the AthenaFusionX repo root.

set -euo pipefail

TARGET="crates/nexus_edge_minion/build.rs"

if [ ! -d "crates/nexus_edge_minion" ]; then
    echo "!! [RPATH-FIX] crates/nexus_edge_minion not found — run this from the repo root."
    exit 1
fi

echo ">> [RPATH-FIX] Writing ${TARGET}..."
cat << 'EOF' > "${TARGET}"
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // crates/nexus_edge_minion -> workspace root -> lib/
    let lib_dir = manifest_dir.join("../../lib");
    let lib_dir = lib_dir.canonicalize().unwrap_or(lib_dir);

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=nexus_loader");
    // Bakes lib_dir into the binary's dynamic section so ld.so finds
    // libnexus_loader.so at runtime without LD_LIBRARY_PATH.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:rerun-if-changed={}/libnexus_loader.so", lib_dir.display());
}
EOF

echo ">> [RPATH-FIX] Rebuilding (rpath is set at link time, so a full rebuild is required)..."
cargo build --features local_edge -p nexus_edge_minion

echo ""
echo ">> [RPATH-FIX] Done. Verifying rpath is embedded..."
BIN="target/debug/nexus_edge_minion"
if command -v readelf >/dev/null 2>&1 && [ -f "${BIN}" ]; then
    readelf -d "${BIN}" | grep -i -E "rpath|runpath" || echo "!! [RPATH-FIX] No RPATH/RUNPATH tag found — check linker output above."
fi

echo ""
echo ">> [RPATH-FIX] Test it in a clean shell (no LD_LIBRARY_PATH exported):"
echo "     env -u LD_LIBRARY_PATH cargo run -p nexus_edge_minion"
echo ""
