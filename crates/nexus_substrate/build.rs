//! build.rs — nexus_substrate v3.0
//! Uses `zig build-lib` directly — no build.zig needed.
//! This is the fastest path on ARM/eMMC hardware (Lenovo Duet).

use std::env;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::current_dir().unwrap();
    let zig_src = manifest_dir.join("src/kernel/muscle.zig");

    println!("cargo:rerun-if-changed=src/kernel/muscle.zig");

    // Derive target triple from Cargo's env vars
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let zig_target = match (arch.as_str(), os.as_str()) {
        ("aarch64", "linux") => "aarch64-linux-musl",
        ("x86_64", "linux") => "x86_64-linux-musl",
        _ => "native",
    };

    let output = Command::new("zig")
        .args([
            "build-lib",
            "-target",
            zig_target,
            "-O",
            "ReleaseFast",
            "-lc",
            "-fcompiler-rt",
            zig_src.to_str().unwrap(),
            &format!("-femit-bin={}/libmuscle.a", out_dir),
            "--name",
            "muscle",
            "-static",
        ])
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "[build.rs] Could not launch `zig`: {}\n\
                 Install zig >= 0.15.2 and ensure it is on PATH.",
                e
            )
        });

    if !output.stdout.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        panic!(
            "[build.rs] Zig compilation failed (exit: {}).\n\
             Source: {}\n\
             See stderr above.",
            output.status,
            zig_src.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=muscle");
}
