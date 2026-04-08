use std::process::Command;
use anyhow::{Context, Result};

pub struct LocalLLM;

impl LocalLLM {
    pub fn new() -> Self { LocalLLM }

    pub fn query_gemma_nano(&self, prompt: &str, model_path: &str) -> Result<String> {
        println!("\x1b[35m>> [LOCAL_LLM] Allocating RAM for {}...\x1b[0m", model_path);
        
        let output = Command::new("./vendor/llama.cpp/llama-cli")
            .args([
                "-m", model_path,
                "-p", prompt,
                "-n", "128", 
                "-c", "512",
                "--temp", "0.1",
                "--log-disable"
            ])
            .output()
            .context("Failed to execute local llama.cpp binary")?;

        if output.status.success() {
            let response = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(response.trim().to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Local GGUF Fault: {}", err))
        }
    }
}
