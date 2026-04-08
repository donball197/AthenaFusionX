// src/orchestrator.rs - Hardened & Cleaned
use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::component_runner::AgentHandle;
use crate::agent_protocol::AgentAction;
use crate::arbiter_agent::ArbiterAgent;
use anyhow::Result;
use std::sync::Arc;

pub struct Orchestrator {
    pub agent: Arc<MicroAgent>,
    handle: AgentHandle,
}

impl Orchestrator {
    pub fn new(agent: Arc<MicroAgent>) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let handle = crate::component_runner::AgentComponent::spawn(
            "orchestrator",
            crate::component_runner::Capabilities::READ_FS | crate::component_runner::Capabilities::WRITE_FS,
            agent.clone(),
            tx,
        );
        Self { agent, handle }
    }

    pub async fn run_autonomous_loop(
        &self,
        instruction: String,
        context: ContextPacket,
        client: &reqwest::Client,
        api_key: &str,
        _tx: &tokio::sync::broadcast::Sender<String>,
        _monitor: &crate::system_monitor::SystemMonitor,
    ) -> Result<String> {
        let (raw_response, _) = self.agent.prepare_and_run(client, api_key, &instruction, context, 1.0)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        match AgentAction::extract_and_parse(&raw_response) {
            Ok(actions) => {
                let arbiter = ArbiterAgent::new(self.agent.clone(), None);
                let action_json = serde_json::to_string(&actions).unwrap_or_default();

                if let Ok(report) = arbiter.analyze_directive(api_key, &action_json).await {
                    if !report.can_proceed || report.risk_score >= 0.45 {
                        return Ok(format!("!! [ARBITER BLOCK] Risk {:.2} — {}", report.risk_score, report.recommendation));
                    }
                }

                let results = self.handle.execute_actions(actions).await?;
                Ok(results.join("\n"))
            }
            Err(e) => {
                println!(">> [ORCHESTRATOR] Parse failed: {}", e);
                Ok(raw_response)
            }
        }
    }
}
