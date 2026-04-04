use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::component_runner::AgentHandle;
use crate::agent_protocol::AgentAction;
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

        // UPGRADE: Using the surgical extractor to ignore markdown fluff and pascal/snake case mismatches
        match AgentAction::extract_and_parse(&raw_response) {
            Ok(actions) => {
                let results: Vec<String> = self.handle.execute_actions(actions).await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                Ok(results.join("\n"))
            }
            Err(e) => {
                println!(">> [ORCHESTRATOR] Protocol extraction failed: {}. Returning raw response.", e);
                Ok(raw_response)
            }
        }
    }
}
