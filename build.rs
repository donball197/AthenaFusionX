use std::process::Command;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rerun-if-changed=src/resonator.zig");
    println!("cargo:rerun-if-changed=ui/dist");

    // Build the Zig static library
    let status = Command::new("zig")
        .args(&[
            "build-lib", 
            "src/resonator.zig", 
            "-lc", 
            "-O", "ReleaseFast",
            &format!("-femit-bin={}/libresonator.a", out_dir)
        ])
        .status()
        .expect("Zig compiler not found. Install with: sudo apt install zig");

    if !status.success() {
        panic!("ZIG_COMPILE_ERROR");
    }

    println!("cargo:rustc-link-lib=static=resonator");
}
