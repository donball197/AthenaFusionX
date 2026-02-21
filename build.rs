use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=resonator.zig");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(&out_dir);

    // Tell Zig to build a static library for ARM64
    let status = Command::new("zig")
        .args(&[
            "build-lib",
            "resonator.zig",
            "-target", "aarch64-linux",
            "-O", "ReleaseFast",
            "-lc",
            &format!("-femit-bin={}/libresonator.a", dest_path.display()),
        ])
        .status()
        .expect("Failed to execute zig compiler. Ensure Zig is in your PATH.");

    if !status.success() {
        panic!("Zig compilation failed");
    }

    println!("cargo:rustc-link-search=native={}", dest_path.display());
    println!("cargo:rustc-link-lib=static=resonator");
}
