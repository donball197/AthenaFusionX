use std::sync::Arc;
use crate::{
    neural_agent::{MicroAgent, ContextPacket},
    swarm::swarm_core::SwarmCore,
};

pub struct Orchestrator {
    pub agent: Arc<MicroAgent>,
    pub swarm: Option<Arc<SwarmCore>>,
}

impl Orchestrator {
    pub fn new(agent: Arc<MicroAgent>, swarm: Option<Arc<SwarmCore>>) -> Self {
        Self { agent, swarm }
    }

    pub async fn run(
        &self,
        input: String,
        ctx: ContextPacket,
        client: &reqwest::Client,
        key: &str,
        tx: Option<tokio::sync::broadcast::Sender<String>>,
    ) -> anyhow::Result<String> {
        // Branch 1: v7 Swarm Intelligence
        if let Some(s) = &self.swarm {
            if let Some(ref t) = tx {
                let _ = t.send("\x1b[1;35m>> [ORCHESTRATOR] Engaging v7 Swarm Consensus...\x1b[0m".into());
            }
            // Passing control to the SwarmCore (Alpha, Beta, Gamma)
            return s.run(input, ctx, client, key).await;
        }

        // Branch 2: Sovereign Fallback (v1.9)
        // Used if the Swarm is uninitialized or in a low-resource state
        if let Some(ref t) = tx {
            let _ = t.send("\x1b[1;33m>> [ORCHESTRATOR] Swarm offline. Falling back to Sovereign Agent.\x1b[0m".into());
        }
        
        let (out, _) = self.agent
            .prepare_and_run(client, key, &input, ctx, 1.0, None)
            .await?;

        Ok(out)
    }
}
