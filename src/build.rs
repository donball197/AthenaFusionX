use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rerun-if-changed=src/resonator.zig");

    // MANDATORY: We tell Zig to build a static library for our specific hardware (ARM NEON)
    let status = Command::new("zig")
        .args(&[
            "build-lib", 
            "src/resonator.zig", 
            "-lc", 
            "-O", "ReleaseFast",
            &format!("-femit-bin={}/libresonator.a", out_dir)
        ])
        .status()
        .expect("CRITICAL: Zig compiler not found. Run 'sudo apt install zig'");

    if !status.success() {
        panic!("ZIG COMPILATION FAILED: The bare-metal resonator could not be manifest.");
    }

    println!("cargo:rustc-link-lib=static=resonator");
}
