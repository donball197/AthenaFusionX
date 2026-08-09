use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

pub struct WasmForge;

/// Run wasm-opt -Oz when Binaryen is available. Returns original bytes on failure.
fn maybe_wasm_opt(wasm_bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::NamedTempFile;

    // Fast path: wasm-opt not installed
    if Command::new("wasm-opt").arg("--version").output().is_err() {
        return wasm_bytes.to_vec();
    }

    let mut input = match NamedTempFile::new() {
        Ok(f) => f,
        Err(_) => return wasm_bytes.to_vec(),
    };
    if input.write_all(wasm_bytes).is_err() {
        return wasm_bytes.to_vec();
    }
    let output_path = input.path().with_extension("opt.wasm");

    let status = Command::new("wasm-opt")
        .args(["-Oz", "--enable-bulk-memory", "--strip-debug", "--strip-producers"])
        .arg(input.path())
        .arg("-o")
        .arg(&output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => std::fs::read(&output_path).unwrap_or_else(|_| wasm_bytes.to_vec()),
        _ => wasm_bytes.to_vec(),
    }
}

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
                "build-exe",
                &tmp_path,
                "-target",
                "wasm32-freestanding",
                "-O",
                "ReleaseSmall",
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

        let wasm_bytes = fs::read("/tmp/cell.wasm").await?;
        let wasm_bytes = maybe_wasm_opt(&wasm_bytes);
        let b64 = general_purpose::STANDARD.encode(&wasm_bytes);
        let _ = fs::remove_file(&tmp_path).await;
        Ok(b64)
    }

    // ⚡ REAL RUST TO WASM PIPELINE
    async fn synthesize_rust(logic_code: &str) -> Result<String> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let tmp_dir = format!("/tmp/nexus_rust_cell_{}", ts);
        fs::create_dir_all(&tmp_dir).await?;

        let cargo_toml = r#"[package]
name = "cell"
version = "0.1.0"
edition = "2021"
[lib]
crate-type = ["cdylib"]
"#;
        fs::write(format!("{}/Cargo.toml", tmp_dir), cargo_toml).await?;

        let src_dir = format!("{}/src", tmp_dir);
        fs::create_dir_all(&src_dir).await?;
        fs::write(format!("{}/lib.rs", src_dir), logic_code).await?;

        let output = tokio::process::Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip1", "--release"])
            .current_dir(&tmp_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = fs::remove_dir_all(&tmp_dir).await;
            return Err(anyhow!("Rust WASM compile error:\n{}", stderr));
        }

        let wasm_bytes =
            fs::read(format!("{}/target/wasm32-wasip1/release/cell.wasm", tmp_dir)).await?;
        let wasm_bytes = maybe_wasm_opt(&wasm_bytes);
        let b64 = general_purpose::STANDARD.encode(&wasm_bytes);
        let _ = fs::remove_dir_all(&tmp_dir).await;
        Ok(b64)
    }
}
