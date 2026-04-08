use std::process::Command;
use crate::nexus_error::NexusError;

pub struct LocalLLM;

impl LocalLLM {
    pub fn new() -> Self { LocalLLM }

    /// Ephemeral Execution Protocol: Load -> Infer -> Drop RAM
    pub fn query_gemma_nano(&self, prompt: &str, model_path: &str) -> Result<String, NexusError> {
        println!("\x1b[35m>> [LOCAL_LLM] Allocating RAM for {}...\x1b[0m", model_path);
        
        // We use llama.cpp's CLI as a universal ARM64 execution wrapper.
        // We restrict context (-c 512) and output tokens (-n 128) to ensure 
        // the process completes in milliseconds and doesn't blow out the RAM.
        let output = Command::new("./vendor/llama.cpp/llama-cli")
            .args([
                "-m", model_path,
                "-p", prompt,
                "-n", "128", 
                "-c", "512",
                "--temp", "0.1",
                "--log-disable" // Ensure stdout only contains the AI's answer
            ])
            .output()
            .map_err(|e| NexusError::Other(format!("Failed to spawn local inference: {}", e)))?;

        if output.status.success() {
            let response = String::from_utf8_lossy(&output.stdout).to_string();
            println!("\x1b[35m>> [LOCAL_LLM] Inference complete. RAM flushed.\x1b[0m");
            Ok(response.trim().to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(NexusError::Other(format!("Local GGUF Fault: {}", err)))
        }
    }
}
