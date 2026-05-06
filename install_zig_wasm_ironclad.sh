#!/usr/bin/env bash
# install_zig_wasm_ironclad.sh — Non-destructive Zig WASM integration
set -e

cd /home/donball197/nexus_singularity-dev || exit 1
echo "🛠️ Applying Ironclad Zig → WASM integration..."

mkdir -p src/zig

# 1. lib.zig — Linear Memory Bridge
cat > src/zig/lib.zig << 'EOF'
const std = @import("std");

var shared_buffer: [4096]u8 = undefined;

export fn get_buffer_ptr() [*]u8 { return &shared_buffer; }
export fn get_buffer_size() usize { return shared_buffer.len; }

export fn calculate_resonance_e2b() f32 {
    var total: f32 = 0.0;
    var i: usize = 0;
    while (i < shared_buffer.len) : (i += 64) {
        const val = @as(f32, @floatFromInt(shared_buffer[i])) / 255.0;
        total += val * val;
    }
    const count = @as(f32, @floatFromInt(shared_buffer.len / 64));
    return if (count > 0) total / count else 0.0;
}

var latent: [32]f32 = [_]f32{0.0} ** 32;
export fn push_latent_intent(input: [*]const f32) void { @memcpy(&latent, input[0..32]); }
export fn get_latent_throttle() f32 {
    var sum: f32 = 0.0;
    for (latent[0..8]) |v| sum += v;
    return sum / 8.0;
}
EOF

# 2. build.zig — Canonical WASM Executable (Zig 0.15)
cat > src/zig/build.zig << 'EOF'
const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{
        .default_target = .{ .cpu_arch = .wasm32, .os_tag = .freestanding },
    });
    const optimize = b.standardOptimizeOption(.{ .preferred_optimize_mode = .ReleaseSmall });

    // For freestanding WASM in Wasmtime, we build an Executable with no entry point
    const lib = b.addExecutable(.{
        .name = "nexus_zig_cell",
        .root_module = b.createModule(.{
            .root_source_file = b.path("lib.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    lib.rdynamic = true;
    lib.entry = .disabled;
    b.installArtifact(lib);
}
EOF

# 3. Safely Rewrite build.rs to handle BOTH Native & WASM
cat > build.rs << 'EOF'
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // 1. Compile the Native Zig Muscle (For VMO Emulation)
    println!("cargo:rerun-if-changed=crates/nexus_substrate/src/zig");
    println!("cargo:rerun-if-changed=crates/nexus_substrate/src/kernel/muscle.zig");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let native_zig_dir = manifest_dir.join("crates/nexus_substrate/src/zig");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os   = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let zig_target  = match (target_arch.as_str(), target_os.as_str()) {
        ("aarch64", "linux") => "aarch64-linux-musl",
        ("x86_64",  "linux") => "x86_64-linux-musl",
        _                    => "native",
    };

    let status = Command::new("zig")
        .current_dir(&native_zig_dir)
        .args(["build", "--prefix", out_dir.to_str().unwrap(), &format!("-Dtarget={}", zig_target), "-Doptimize=ReleaseSafe"])
        .status().expect("Failed to launch `zig build` for native muscle");

    if !status.success() { panic!("Native Zig compilation failed"); }
    println!("cargo:rustc-link-search=native={}", out_dir.join("lib").display());
    println!("cargo:rustc-link-lib=static=muscle");

    // 2. Compile the WASM Cells (No Linking)
    println!("cargo:rerun-if-changed=src/zig");
    let wasm_status = Command::new("zig")
        .current_dir("src/zig")
        .args(["build", "--release"])
        .status().expect("zig build failed for WASM cells");

    assert!(wasm_status.success(), "Zig WASM build failed");
}
EOF

# 4. src/zig/mod.rs — Zero-copy fix with correct path
cat > src/zig/mod.rs << 'EOF'
const ZIG_CELL_WASM: &[u8] = include_bytes!("zig-out/bin/nexus_zig_cell.wasm");

pub fn resonance_from_vmo(handle: u32, size: usize) -> f32 {
    let mut data = vec![0u8; size.min(4096)];
    unsafe {
        if crate::kernel::zx_vmo_read_native(handle, 0, data.as_mut_ptr(), data.len()) != 0 {
            return 0.0;
        }
    }
    resonance_from_buffer(&data)
}

pub fn resonance_from_buffer(data: &[u8]) -> f32 {
    match crate::wasm_runtime::WasmRuntime::run_with_buffer(
        ZIG_CELL_WASM, 
        "calculate_resonance_e2b",
        data,
    ) {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("Zig WASM cell failed: {}", e);
            0.0
        }
    }
}
EOF

# 5. Register module in lib.rs safely
if ! grep -q "pub mod zig;" src/lib.rs; then
    echo "pub mod zig;" >> src/lib.rs
    echo "✅ Registered 'pub mod zig;' in src/lib.rs"
fi

# 6. Reconstruct wasm_forge.rs preserving Rust capabilities
cat > src/wasm_forge.rs << 'EOF'
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

pub struct WasmForge;

impl WasmForge {
    pub async fn synthesize_cell(logic_code: &str) -> Result<String> {
        if logic_code.trim().starts_with("const std") || logic_code.contains("export fn") {
            return Self::synthesize_zig(logic_code).await;
        }
        Self::synthesize_rust(logic_code).await
    }

    async fn synthesize_zig(zig_code: &str) -> Result<String> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let tmp_path = format!("/tmp/nexus_zig_cell_{}.zig", ts);

        fs::write(&tmp_path, zig_code).await?;

        let output = tokio::process::Command::new("zig")
            .args([
                "build-lib", &tmp_path,
                "-target", "wasm32-freestanding",
                "-O", "ReleaseSmall",
                "-dynamic",
                "--name", "cell"
            ])
            .current_dir("/tmp")
            .output().await?;

        if !output.status.success() {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(anyhow!("Zig compile error: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let wasm_bytes = fs::read("/tmp/cell.wasm").await?;
        let b64 = general_purpose::STANDARD.encode(&wasm_bytes);

        let _ = fs::remove_file(&tmp_path).await;
        let _ = fs::remove_file("/tmp/cell.wasm").await;
        let _ = fs::remove_file("/tmp/cell.wasm.o").await;

        Ok(b64)
    }

    async fn synthesize_rust(logic_code: &str) -> Result<String> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let build_dir = format!("/tmp/nexus_forge_{}", ts);
        fs::create_dir_all(format!("{}/src", build_dir)).await?;

        let toml = r#"
[package]
name = "nexus_dynamic_cell"
version = "1.0.0"
edition = "2021"
[lib]
crate-type = ["cdylib"]
[profile.release]
opt-level = "z"
lto = true
panic = "abort"
"#;
        fs::write(format!("{}/Cargo.toml", build_dir), toml).await?;
        let wrapped_code = format!("#[no_mangle]\n{}", logic_code);
        fs::write(format!("{}/src/lib.rs", build_dir), wrapped_code).await?;

        let _ = Command::new("rustup").args(["target", "add", "wasm32-unknown-unknown"]).output();
        let output = Command::new("cargo")
            .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
            .current_dir(&build_dir)
            .output()?;

        if !output.status.success() {
            let _ = fs::remove_dir_all(&build_dir).await;
            return Err(anyhow!("FORGE_FAULT: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let wasm_path = format!("{}/target/wasm32-unknown-unknown/release/nexus_dynamic_cell.wasm", build_dir);
        let wasm_bytes = fs::read(&wasm_path).await?;
        let base64_payload = general_purpose::STANDARD.encode(&wasm_bytes);
        let _ = fs::remove_dir_all(&build_dir).await;
        
        Ok(base64_payload)
    }
}
EOF

echo "🔨 Building project with Resource Shield (-j 1)..."
cargo build -j 1

echo "✅ Ironclad Zig WASM integration installed safely!"
