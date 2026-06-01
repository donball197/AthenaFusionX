use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::fs;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = env::current_dir().unwrap();
    let zig_src = manifest_dir.join("src/kernel/muscle.zig");
    
    println!("cargo:rerun-if-changed=src/kernel/muscle.zig");

    let temp_lib = manifest_dir.join("libmuscle.a");
    
    let status = Command::new("zig")
        .args([
            "build-lib",
            "-target", "aarch64-linux-gnu",
            "-O", "ReleaseSmall",
            "-lc",
            "-fPIC",
            zig_src.to_str().unwrap(),
            &format!("-femit-bin={}", temp_lib.to_str().unwrap()),
        ])
        .status()
        .expect("Failed to execute zig");

    if !status.success() {
        panic!("Zig compilation failed");
    }

    let dest_lib = out_dir.join("libmuscle.a");
    fs::copy(&temp_lib, &dest_lib).expect("Failed to copy libmuscle.a");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Wl,--whole-archive");
    println!("cargo:rustc-link-lib=static=muscle");
    println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");
}
