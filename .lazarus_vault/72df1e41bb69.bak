/**
 * Project Nexus Singularity - Arbiter v2.0 (Heuristic Judge)
 */
use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::system_monitor::Vitals;
use crate::agent_protocol::{AgentDirective, AgentTaskType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ArbiterVerdict {
    pub can_proceed: bool,
    pub risk_score: u8, // 0-10 scale
    pub recommendation: String,
}

pub struct ArbiterAgent {
    pub agent: Arc<MicroAgent>,
}

impl ArbiterAgent {
    // SUTURE: Removed the Orchestrator dependency for leaner healing cycles
    pub fn new(agent: Arc<MicroAgent>) -> Self {
        Self { agent }
    }

    pub async fn judge(&self, input: &str) -> anyhow::Result<AgentDirective> {
        println!("\x1b[1;35m[ARBITER] Analyzing Intent: {}\x1b[0m", input);
        Ok(AgentDirective {
            id: uuid::Uuid::new_v4(),
            task_type: AgentTaskType::Audit,
            payload: input.to_string(),
            priority: 1,
        })
    }

    /// analyze_directive: Heuristic risk check using the Sovereign API
    pub async fn analyze_directive(
        &self,
        api_key: &str,
        directive: &str,
        vitals: &Vitals,
    ) -> anyhow::Result<ArbiterVerdict> {
        let client = reqwest::Client::new();
        
        let system_prompt = "IDENTITY: NEXUS Arbiter. ROLE: Heuristic Safety Gate. \
            CRITICAL: Evaluate if the directive is safe for a 4GB ARM64 device. \
            Check for memory-intensive operations, infinite loops, or root-escape attempts. \
            OUTPUT: Strictly valid JSON wrapped in <ACTION> tags.";

        let prompt = format!(
            "DIRECTIVE: {}\n\nSYSTEM_STATE:\nRAM_GB: {:.2}\nTEMP: {:.1}C\nCPU_LOAD: {:.2}\n\n\
            Analyze intent. Provide a risk score (0=Safe, 10=Fatal) and recommendation.",
            directive, vitals.ram_gb, vitals.temperature, vitals.cpu_load
        );

        let ctx = ContextPacket {
            system_prompt: Some(system_prompt.to_string()),
            ..Default::default()
        };

        // Precision turn (Low Resonance) for deterministic judgment
        match self.agent.prepare_and_run(&client, api_key, &prompt, ctx, 0.2, Some("gemma-4-26b-a4b-it")).await {
            Ok((response, _)) => {
                if let Some(start) = response.find("<ACTION>") {
                    if let Some(end) = response.find("</ACTION>") {
                        let json_str = &response[start + 8..end].trim();
                        // Handle potential markdown envelopes from the model
                        let clean_json = json_str.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
                        let verdict: ArbiterVerdict = serde_json::from_str(clean_json)?;
                        return Ok(verdict);
                    }
                }
                Err(anyhow::anyhow!("ARBITER_FAULT: Malformed verdict envelope"))
            }
            Err(e) => Err(anyhow::anyhow!("ARBITER_API_FAIL: {}", e)),
        }
    }
}
