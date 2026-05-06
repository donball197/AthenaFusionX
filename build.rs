use std::path::PathBuf;
use std::process::Command;

fn main() {
    // 1. Compile the Native Zig Muscle (For VMO Emulation)
    println!("cargo:rerun-if-changed=crates/nexus_substrate/src/zig");
    println!("cargo:rerun-if-changed=crates/nexus_substrate/src/kernel/muscle.zig");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let native_zig_dir = manifest_dir.join("crates/nexus_substrate/src/zig");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os   = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let zig_target  = match (target_arch.as_str(), target_os.as_str()) {
        ("aarch64", "linux") => "aarch64-linux-musl",
        ("x86_64",  "linux") => "x86_64-linux-musl",
        _                    => "native",
    };

    let status = Command::new("zig")
        .current_dir(&native_zig_dir)
        .args(["build", "--prefix", out_dir.to_str().unwrap(), &format!("-Dtarget={}", zig_target), "-Doptimize=ReleaseSafe"])
        .status().expect("Failed to launch `zig build` for native muscle");

    if !status.success() { panic!("Native Zig compilation failed"); }
    println!("cargo:rustc-link-search=native={}", out_dir.join("lib").display());
    println!("cargo:rustc-link-lib=static=muscle");

    // 2. Compile the WASM Cells (No Linking)
    println!("cargo:rerun-if-changed=src/zig");
    let wasm_status = Command::new("zig")
        .current_dir("src/zig")
        .args(["build", "--release"])
        .status().expect("zig build failed for WASM cells");

    assert!(wasm_status.success(), "Zig WASM build failed");
}
