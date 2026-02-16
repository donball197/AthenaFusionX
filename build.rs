use std::process::Command;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    
    // Check for Zig Resonator
    if std::path::Path::new("resonator.zig").exists() {
        println!("cargo:rerun-if-changed=resonator.zig");
        let status = Command::new("zig")
            .args(&["build-lib", "resonator.zig", "-dynamic", "-femit-bin=libresonator.so"])
            .current_dir(".")
            .status();

        if status.is_ok() {
             println!("cargo:rustc-link-search=native=.");
             println!("cargo:rustc-link-lib=dylib=resonator");
        } else {
             println!("cargo:warning=Zig compiler not found. Swarm functions will be mocked.");
        }
    }
}
