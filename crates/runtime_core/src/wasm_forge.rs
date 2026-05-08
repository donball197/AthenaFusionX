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

        // ⚡ ZIG 0.15 WASM COMPILATION FIX (No Linker Panics)
        let output = tokio::process::Command::new("zig")
            .args([
                "build-exe",
                &tmp_path,
                "-target",
                "wasm32-freestanding",
                "-O",
                "ReleaseSmall",
                "-fno-entry",
                "-rdynamic",
                "--name",
                "cell",
            ])
            .current_dir("/tmp")
            .output()
            .await?;

        if !output.status.success() {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(anyhow!("Zig compile error: {}", String::from_utf8_lossy(&output.stderr)));
        }

        // Zig 0.15 writes cell.wasm to its own cwd, not /tmp
        // Try /tmp first, then fall back to current dir
        let wasm_path = if tokio::fs::metadata("/tmp/cell.wasm").await.is_ok() {
            "/tmp/cell.wasm".to_string()
        } else {
            "cell.wasm".to_string()
        };
        let wasm_bytes = fs::read(&wasm_path).await?;
        let b64 = general_purpose::STANDARD.encode(&wasm_bytes);

        let _ = fs::remove_file(&tmp_path).await;
        let _ = fs::remove_file("/tmp/cell.wasm").await;
        let _ = fs::remove_file("/tmp/cell.wasm.o").await;
        let _ = fs::remove_file("cell.wasm").await;

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

        let wasm_path =
            format!("{}/target/wasm32-unknown-unknown/release/nexus_dynamic_cell.wasm", build_dir);
        let wasm_bytes = fs::read(&wasm_path).await?;
        let base64_payload = general_purpose::STANDARD.encode(&wasm_bytes);
        let _ = fs::remove_dir_all(&build_dir).await;

        Ok(base64_payload)
    }
}
