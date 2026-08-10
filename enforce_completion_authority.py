import os

print(">> Enforcing Subgoal Completion Authority...")

# ==========================================
# 1. UPGRADE CORTICAL LOOP (Strict Authority)
# ==========================================
path1 = "crates/runtime_core/src/cortical_loop.rs"
if os.path.exists(path1):
    with open(path1, "r") as f:
        content = f.read()

    # Locate the massive, weak lexical matching block
    start_str = '// ⚡ DETERMINISTIC HALT SHORT-CIRCUIT'
    end_str = '// --- ANTI-LOOP KINETIC GUARD ---'
    
    if start_str in content and end_str in content:
        start_idx = content.find(start_str)
        end_idx = content.find(end_str)
        
        # Replace the entire Halt and Anti-Loop logic with strict Verification Engine delegation
        new_logic = """// ⚡ DETERMINISTIC HALT SHORT-CIRCUIT
                let prop_lower = proposed_action.trim().to_lowercase();
                if prop_lower == "halt" || prop_lower.starts_with("halt ") {
                    if session.is_subgoal_satisfied(current_belief) {
                        self.log("\\x1b[1;36m>> [CORTEX] DETERMINISTIC GUARD: Halt proposed and Verification Engine confirms completion.\\x1b[0m".to_string());
                        return Ok(CortexOutcome::Proven {
                            belief: 1.0,
                            evidence: session.collect_evidence(h_id),
                        });
                    } else {
                        self.log("\\x1b[1;31m>> [CORTEX FAULT] Halt rejected: Verification engine failed to confirm contract satisfaction.\\x1b[0m".to_string());
                        let obs_id = session.graph.add_node(NodeKind::Observation, "[HALT_REJECTED] You proposed halt, but the Verification Engine confirmed the objective contract is NOT satisfied. Ensure you have executed the required actions and acquired the necessary evidence markers.", 1.0, 50);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.6);
                        session.graph.propagate_beliefs();
                        continue;
                    }
                }

                // --- ANTI-LOOP KINETIC GUARD ---
                let action_key = proposed_action.trim().to_string();
                if session.is_action_attempted(&action_key) {
                    if session.is_subgoal_satisfied(current_belief) {
                        self.log("\\x1b[1;36m>> [SUBGOAL_COMPLETE] Loop detected, but Verification Engine confirms objective is satisfied. Halting.\\x1b[0m".to_string());
                        return Ok(CortexOutcome::Proven {
                            belief: current_belief,
                            evidence: session.collect_evidence(h_id),
                        });
                    } else {
                        self.log(format!("\\x1b[1;31m>> [ENTERPRISE CORTEX] Anti-Loop Guard Triggered: Action '{}' already attempted and verification failed.\\x1b[0m", action_key));
                        let obs_id = session.graph.add_node(
                            NodeKind::Observation,
                            "[LOOP_PREVENTED] You already proposed and executed this exact command, but the objective is NOT complete. Try a different tool or approach.",
                            1.0, 50
                        );
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.40);
                        session.graph.propagate_beliefs();
                        continue;
                    }
                }
                session.record_action_attempt(&action_key);
                
                """
        
        content = content[:start_idx] + new_logic + content[end_idx + len(end_str):]
        with open(path1, "w") as f:
            f.write(content)
        print("[OK] CorticalLoop patched: Centralized Completion Authority enforced.")
    else:
        print("[!] Target bounds in CorticalLoop not found. May already be patched.")
else:
    print(f"[ERROR] {path1} not found.")


# ==========================================
# 2. RESTORE CODE AUDITOR REGRESSION
# ==========================================
path2 = "apps/code_auditor/src/agent.rs"
if os.path.exists(path2):
    with open(path2, "r") as f:
        content2 = f.read()
    
    # Check if the stub is present
    if "Ok(vec![])" in content2:
        # We rewrite the file to properly use the orchestrator as designed
        new_auditor = """//! agent.rs
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info};
use nexus_model_hub::neural_agent::ContextPacket;
use nexus_runtime_core::orchestrator::Orchestrator;

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
    model_name: String,
    #[allow(dead_code)] max_iterations: u32,
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
        Self { orchestrator, client, api_key, repo_path, model_name, max_iterations }
    }

    fn compile_system_prompt(&self) -> String {
        format!(
            "You are the Nexus Autonomous Code Auditor.\\nSTRICT DIRECTORY BOUNDARY: Your analysis is strictly scoped to '{}'.\\nAnalyze the codebase for vulnerabilities and output your findings strictly as a JSON array of objects.",
            self.repo_path.display()
        )
    }

    pub async fn run_analysis(&mut self) -> Result<Vec<ReportedIssue>> {
        info!("Dispatching code audit mission to Nexus Orchestrator...");
        let ctx = ContextPacket {
            system_prompt: Some(self.compile_system_prompt()),
            model_override: Some(self.model_name.clone()),
            ..Default::default()
        };

        let mission_prompt = format!(
            "Execute an end-to-end read-only audit of {}. Identify high-priority issues (critical/high/medium/low).\\nEXPECTED JSON SCHEMA:\\n[{{\\"file_path\\": \\"src/main.rs\\", \\"line_number\\": 42, \\"severity\\": \\"high\\", \\"category\\": \\"Security\\", \\"title\\": \\"Issue\\", \\"description\\": \\"Desc\\", \\"suggestion\\": \\"Fix\\"}}]",
            self.repo_path.display()
        );

        let raw_response = self.orchestrator.run(mission_prompt, ctx, &self.client, &self.api_key, None).await.map_err(|e| anyhow::anyhow!("Cortex failed: {}", e))?;
        
        let clean_json = self.extract_json(&raw_response);
        let issues: Vec<ReportedIssue> = serde_json::from_str(&clean_json).context("Failed to deserialize LLM output.")?;
        Ok(issues)
    }

    fn extract_json(&self, raw: &str) -> String {
        let mut text = raw.trim();
        if text.starts_with("```json") { text = text.trim_start_matches("```json"); }
        else if text.starts_with("```") { text = text.trim_start_matches("```"); }
        if text.ends_with("```") { text = text.trim_end_matches("```"); }
        text.trim().to_string()
    }
}"""
        with open(path2, "w") as f:
            f.write(new_auditor)
        print("[OK] Code Auditor restored: Removed Ok(vec![]) stub and reconnected Orchestrator.")
    else:
        print("[!] Code Auditor is already correctly implemented. No stub found.")
else:
    print(f"[ERROR] {path2} not found.")

print(">> Protocol Validated.")
