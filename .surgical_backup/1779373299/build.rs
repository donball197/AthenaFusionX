use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/kernel/muscle.zig");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let output_lib = out_dir.join("libmuscle.a");

    let status = Command::new("zig")
        .args([
            "build-lib",
            "-O",
            "ReleaseFast",
            "-static",
            "-fPIC",
            "-femit-bin",
            output_lib.to_str().unwrap(),
            "src/kernel/muscle.zig",
        ])
        .status()
        .expect("failed to execute zig compiler");

    if !status.success() {
        panic!("zig compilation failed");
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());

    // Force linker retention for all Zig exports
    println!("cargo:rustc-link-arg=-Wl,--whole-archive");
    println!("cargo:rustc-link-lib=static=muscle");
    println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");

    // ARM64 Chromebook / Crostini stabilization
    println!("cargo:rustc-link-lib=c");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=rt");

    // Explicit ARM64 libc search paths
    println!("cargo:rustc-link-search=native=/usr/lib/aarch64-linux-gnu");
    println!("cargo:rustc-link-search=native=/lib/aarch64-linux-gnu");
}
