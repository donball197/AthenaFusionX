use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=resonator.zig");
    
    let out_dir = env::var("OUT_DIR").unwrap();

    let status = Command::new("zig")
        .args(&[
            "build-lib", 
            "resonator.zig", 
            "-static", 
            "-O", "ReleaseSmall", 
            &format!("-femit-bin={}/libresonator.a", out_dir)
        ])
        .current_dir(".") 
        .status()
        .expect("Failed to execute Zig. Is 'zig' in your PATH?");

    if !status.success() {
        panic!("Zig compilation failed!");
    }

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=resonator");
}
