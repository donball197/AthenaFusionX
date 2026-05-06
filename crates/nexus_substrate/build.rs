use std::process::Command;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::current_dir().unwrap();
    let zig_src = manifest_dir.join("src/kernel/muscle.zig");

    println!("cargo:rerun-if-changed=src/kernel/muscle.zig");

    // ⚡ Compile Zig source into a static library
    let status = Command::new("zig")
        .args([
            "build-lib",
            "-O", "ReleaseFast",
            "-lc",
            zig_src.to_str().unwrap(),
            &format!("-femit-bin={}/libmuscle.a", out_dir),
            "--name", "muscle",
        ])
        .status()
        .expect(">> [BUILD_ERROR] Zig compiler not found. Install zig to build the hardware bridge.");

    if !status.success() {
        panic!(">> [BUILD_ERROR] Zig compilation failed.");
    }

    // ⚡ Tell Cargo where to find the library
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=muscle");
}
