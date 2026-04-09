use std::process::Command;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=src/reflex/extractor.zig");

    let status = Command::new("zig")
        .args([
            "build-obj", "src/reflex/extractor.zig",
            "-O", "ReleaseSmall", "-fPIC",
            &format!("-femit-bin={}/extractor.o", out_dir),
        ])
        .status()
        .expect("Zig compiler missing.");

    if !status.success() { panic!("ZIG_COMPILE_ERROR"); }

    Command::new("ar")
        .args(["crus", &format!("{}/libextractor.a", out_dir), &format!("{}/extractor.o", out_dir)])
        .status().ok();

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=extractor");
}
