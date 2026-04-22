use anyhow::{anyhow, Result};
use std::process::Command;
use tokio::fs;
use base64::{engine::general_purpose, Engine as _};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct WasmForge;

impl WasmForge {
    pub async fn synthesize_cell(logic_code: &str) -> Result<String> {
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

        // ⚡ V14: Training wheels removed. Direct passthrough.
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
