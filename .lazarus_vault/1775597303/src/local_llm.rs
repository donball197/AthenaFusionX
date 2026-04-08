use std::process::Command;
use anyhow::{Context, Result, anyhow};
use crate::dynamic_broker::DynamicBroker;

pub struct LocalLLM;

impl LocalLLM {
    pub fn new() -> Self { LocalLLM }

    pub fn query_gemma_nano(&self, prompt: &str, model_path: &str) -> Result<String> {
        // SOVEREIGN RESOLUTION: Determine binary location based on host muscle
        // We look for 'llama-cli' in the device-specific bin directory
        let binary_path = DynamicBroker::get_library_path("llama-cli")
            .context("Failed to resolve local inference binary for this host")?;

        println!("\x1b[35m>> [LOCAL_LLM] Allocating RAM for {} using {:?}...\x1b[0m", model_path, binary_path);
        
        let output = Command::new(binary_path)
            .args([
                "-m", model_path,
                "-p", prompt,
                "-n", "128", 
                "-c", "512",
                "--temp", "0.1",
                "--log-disable"
            ])
            .output()
            .context("Failed to execute local inference process")?;

        if output.status.success() {
            let response = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(response.trim().to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Local GGUF Fault: {}", err))
        }
    }
}
