#!/usr/bin/env bash
set -euo pipefail

echo "======================================================"
echo " Seating Refactored agent.rs into Code Auditor"
echo "======================================================"

mkdir -p apps/code_auditor/src

cat > apps/code_auditor/src/agent.rs << 'EOF'
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
    orchestrator: Arc<Orchestrator>,
    client: Client,
    api_key: String,
    repo_path: PathBuf,
    model_name: String, // Dynamically pulled from config, NEVER hardwired.
    max_iterations: u32,
}

impl CodeAnalysisAgent {
    pub fn new(
        orchestrator: Arc<Orchestrator>,
        client: Client,
        api_key: String,
        repo_path: PathBuf,
        model_name: String,
        max_iterations: u32,
    ) -> Self {
        Self {
            orchestrator,
            client,
            api_key,
            repo_path,
            model_name,
            max_iterations,
        }
    }

    /// Compiles the cognitive context for the sub-agent, locking it to the target directory.
    fn compile_system_prompt(&self) -> String {
        format!(
            "You are the Nexus Autonomous Code Auditor.\n\
            STRICT DIRECTORY BOUNDARY: Your analysis is strictly scoped to the repository mounted at '{}'.\n\
            You have full access to workspace read tools (e.g., read_file, symbol_graph). \n\
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
        let ctx = ContextPacket {
            system_prompt: Some(self.compile_system_prompt()),
            model_override: Some(self.model_name.clone()),
            ..Default::default()
        };

        // Formulate the mission directive for the Executive Cortex
        let mission_prompt = format!(
            "Execute an end-to-end read-only audit of the repository located at {}.\n\
            1. Use workspace tools to explore the architecture and read critical source files.\n\
            2. Identify high-priority issues, categorizing severity as critical, high, medium, or low.\n\
            3. When you have gathered sufficient evidence, HALT and output your final report as a RAW JSON array.\n\
            \n\
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
            self.repo_path.display()
        );

        // Ignite the DAG loop via the Orchestrator
        let raw_response = self
            .orchestrator
            .run(&mission_prompt, ctx, &self.client, &self.api_key, None)
            .await
            .context("Executive Cortex failed to execute the audit mission")?;

        // Extract and sanitize the JSON (in case the model disobeys formatting rules)
        let clean_json = self.extract_json(&raw_response);

        // Deserialize the LLM output directly into the structs required by main.rs
        let issues: Vec<ReportedIssue> = serde_json::from_str(&clean_json)
            .context("Failed to deserialize LLM output. Model may have hallucinated the JSON schema.")?;

        Ok(issues)
    }

    /// Strips markdown fences if the model aggressively formats the JSON output.
    fn extract_json(&self, raw: &str) -> String {
        let mut text = raw.trim();
        if text.starts_with("```json") {
            text = text.trim_start_matches("```json");
        } else if text.starts_with("```") {
            text = text.trim_start_matches("```");
        }
        if text.ends_with("```") {
            text = text.trim_end_matches("```");
        }
        text.trim().to_string()
    }
}
EOF

echo ">> Purging stale build cache for ARM64 shared DNA safety..."
rm -rf target/
rm -f *.so 2>/dev/null || true

echo ">> Compiling Code Auditor in universal src/ architecture..."
cargo build --release -p code_auditor

echo "======================================================"
echo " [SUCCESS] agent.rs seated and compiled."
echo "======================================================"
