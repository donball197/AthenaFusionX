use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let obj_path = format!("{}/resonator.o", out_dir.display());

    // 1. Forge a RAW OBJECT FILE instead of an archive
    let status = std::process::Command::new("zig")
        .args([
            "build-obj",
            "src/resonator.zig",
            "-lc",
            "-O",
            "ReleaseFast",
            &format!("-femit-bin={}", obj_path),
        ])
        .status()
        .expect("Zig forge failed.");

    if !status.success() {
        panic!("ZIG_COMPILE_ERROR");
    }

    // 2. FORCE the object file directly into the linker
    println!("cargo:rustc-link-arg={}", obj_path);

    // 3. Link llama.cpp and C++ libs
    println!("cargo:rustc-link-search=native=./vendor/llama.cpp/build/src");
    println!("cargo:rustc-link-search=native=./vendor/llama.cpp/build/ggml/src");
    println!("cargo:rustc-link-lib=static=llama");
    println!("cargo:rustc-link-lib=static=ggml");
    println!("cargo:rustc-link-lib=dylib=stdc++");

    println!("cargo:rerun-if-changed=src/resonator.zig");
}
