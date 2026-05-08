use crate::neural_agent::{ContextPacket, MicroAgent};
use std::sync::Arc;

pub struct SwarmNode {
    pub name: String,
    pub agent: Arc<MicroAgent>,
}

impl SwarmNode {
    pub fn new(name: &str, agent: Arc<MicroAgent>) -> Self {
        Self { name: name.to_string(), agent }
    }

    pub async fn run(
        &self,
        input: String,
        ctx: ContextPacket,
        client: &reqwest::Client,
        key: &str,
    ) -> anyhow::Result<String> {
        println!("\x1b[1;32m[NODE {}] Processing Pulse...\x1b[0m", self.name);
        // Each node uses the shared agent but runs its own inference task
        let (output, _) = self.agent.prepare_and_run(client, key, &input, ctx, 1.0, None).await?;
        Ok(output)
    }
}
