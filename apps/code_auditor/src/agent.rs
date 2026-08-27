//! agent.rs
//!
//! Hollowed-out Code Analysis Agent that bridges the standalone Code Auditor
//! into the Nexus Singularity Executive Cortex.
//!
//! Bypasses local inference entirely to enforce Zero Fragmentation across ARM64.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info};
use walkdir::WalkDir;

// Import Nexus Substrate primitives
use nexus_model_hub::neural_agent::ContextPacket;
use nexus_runtime_core::orchestrator::Orchestrator;

/// The structured output expected by the Code Auditor's main reporting loop.
/// The Epistemic Engine will be instructed to yield exactly this schema.
#[derive(Debug, Deserialize, Serialize)]
pub struct ReportedIssue {
    pub file_path: String,
    pub line_number: u32,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub suggestion: String,
}

pub struct CodeAnalysisAgent {
    #[allow(dead_code)]
    orchestrator: Arc<Orchestrator>,
    client: Client,
    api_key: String,
    repo_path: PathBuf,
    model_name: String, // Dynamically pulled from config, NEVER hardwired.
    #[allow(dead_code)]
    max_iterations: u32,
}

impl CodeAnalysisAgent {
    pub fn new(
        #[allow(dead_code)] orchestrator: Arc<Orchestrator>,
        client: Client,
        api_key: String,
        repo_path: PathBuf,
        model_name: String,
        #[allow(dead_code)] max_iterations: u32,
    ) -> Self {
        Self { orchestrator, client, api_key, repo_path, model_name, max_iterations }
    }

    /// Compiles the cognitive context for the sub-agent, locking it to the target directory.
    fn compile_system_prompt(&self) -> String {
        format!(
            "You are the Nexus Autonomous Code Auditor.\n\
STRICT DIRECTORY BOUNDARY: Your analysis is strictly scoped to the repository mounted at '{}'.\n\n\
Analyze the codebase for security vulnerabilities, logical flaws, and performance bottlenecks.\n\
You MUST synthesize your final findings strictly as a JSON array of objects.",
            self.repo_path.display()
        )
    }

    /// Triggers the cognitive loop via the Orchestrator and parses the LLM's final response.
    pub async fn run_analysis(&mut self) -> Result<Vec<ReportedIssue>> {
        info!("Dispatching code audit mission to Nexus Orchestrator...");
        debug!("Target path: {}", self.repo_path.display());
        debug!("Dynamic model override: {}", self.model_name);

        // Build the context packet using the dynamic model and our isolated prompt
        let _ctx = ContextPacket {
            system_prompt: Some(self.compile_system_prompt()),
            model_override: Some(self.model_name.clone()),
            ..Default::default()
        };

        // Formulate the mission directive for the Executive Cortex
        let mut repo_contents = String::new();
        for entry in WalkDir::new(&self.repo_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let path_str = path.to_string_lossy();
                if path_str.contains("/.git/")
                    || path_str.contains("/target/")
                    || path_str.contains("__pycache__")
                {
                    continue;
                }
                if let Ok(file_content) = std::fs::read_to_string(path) {
                    repo_contents
                        .push_str(&format!("\n--- FILE: {} ---\n{}\n", path_str, file_content));
                }
            }
        }

        let mission_prompt = format!(
            "Execute an end-to-end read-only audit of the repository located at {}.\n\n\
1. Read the provided source files and explore the architecture.\n\
2. Identify high-priority issues, categorizing severity as critical, high, medium, or low.\n\
3. Output your final report as a RAW JSON array.\n\n\
REPOSITORY SOURCE CODE:\n\
{}\n\n\
EXPECTED JSON SCHEMA:\n\
[\n\
  {{\n\
    \"file_path\": \"src/main.rs\",\n\
    \"line_number\": 42,\n\
    \"severity\": \"high\",\n\
    \"category\": \"Security\",\n\
    \"title\": \"Unsafe memory access\",\n\
    \"description\": \"Detailed explanation here.\",\n\
    \"suggestion\": \"Use safe Rust idioms.\"\n\
  }}\n\
]\n\
Do NOT wrap the final JSON array in markdown formatting like ```json.",
            self.repo_path.display(),
            repo_contents
        );

        // Direct API Dispatch (Bypassing orchestrator tool-loop friction for Code Auditor)
        let endpoint = if self.model_name.starts_with("nvidia/")
            || self.model_name.starts_with("deepseek-ai/")
        {
            "https://integrate.api.nvidia.com/v1/chat/completions"
        } else {
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
            // or standard Gemini endpoint
        };

        let body = serde_json::json!({
            "model": self.model_name,
            "messages": [
                {"role": "system", "content": self.compile_system_prompt()},
                {"role": "user", "content": mission_prompt}
            ],
            "temperature": 0.1,
            "max_tokens": 4096
        });

        let res = self
            .client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to LLM provider")?;

        let status = res.status();
        let resp_text = res.text().await.context("Failed to read LLM response body")?;

        if !status.is_success() {
            anyhow::bail!("API error (status {}): {}", status, resp_text);
        }

        // Parse OpenAI/NVIDIA compatible chat completion response
        let v: serde_json::Value =
            serde_json::from_str(&resp_text).context("Failed to parse chat completion JSON")?;

        let raw_response = v["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!("Response missing choices[0].message.content: {}", resp_text)
            })?
            .to_string();

        // Extract and sanitize the JSON (in case the model disobeys formatting rules)

        println!("\n=== [DEBUG] RAW ORCHESTRATOR RESPONSE ===");
        println!("{}", raw_response);
        println!("===========================================\n");

        let clean_json = self.extract_json(&raw_response);

        // Deserialize the LLM output directly into the structs required by main.rs
        let issues: Vec<ReportedIssue> = serde_json::from_str(&clean_json).context(
            "Failed to deserialize LLM output. Model may have hallucinated the JSON schema.",
        )?;

        Ok(issues)
    }

    /// Strips markdown fences if the model aggressively formats the JSON output.
    fn extract_json(&self, raw: &str) -> String {
        if let (Some(start), Some(end)) = (raw.find('['), raw.rfind(']')) {
            if start <= end {
                return raw[start..=end].to_string();
            }
        }
        // Fallback in case it's completely malformed
        raw.trim().to_string()
    }
}
