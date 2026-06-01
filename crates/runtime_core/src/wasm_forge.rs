use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

pub struct WasmForge;

impl WasmForge {
    // Pure local synthesis only.
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
            .args(["build-exe", &tmp_path, "-target", "wasm32-freestanding", "-O", "ReleaseSmall", "--name", "cell"])
            .current_dir("/tmp")
            .output().await?;

        if !output.status.success() {
            let _ = fs::remove_file(&tmp_path).await;
            return Err(anyhow!("Zig compile error"));
        }

        let wasm_bytes = fs::read("/tmp/cell.wasm").await?;
        let b64 = general_purpose::STANDARD.encode(&wasm_bytes);
        let _ = fs::remove_file(&tmp_path).await;
        Ok(b64)
    }

    async fn synthesize_rust(_logic_code: &str) -> Result<String> {
        Err(anyhow!("Rust WASM synthesis stubbed for build"))
    }
}
