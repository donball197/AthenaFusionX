use crate::{
    swarm::swarm_node::SwarmNode,
    swarm::consensus::ConsensusEngine,
    swarm::arbiter::Arbiter,
    neural_agent::ContextPacket,
};

pub struct SwarmCore {
    pub nodes: Vec<SwarmNode>,
}

impl SwarmCore {
    pub fn new(nodes: Vec<SwarmNode>) -> Self {
        Self { nodes }
    }

    pub async fn run(
        &self,
        input: String,
        ctx: ContextPacket,
        client: &reqwest::Client,
        key: &str,
    ) -> anyhow::Result<String> {
        let mut outputs = Vec::new();

        // Sequential execution to protect the 4GB RAM Prison
        for n in &self.nodes {
            outputs.push(n.run(input.clone(), ctx.clone(), client, key).await?);
        }

        let consensus = ConsensusEngine::resolve(outputs.clone());
        let best = Arbiter::best(outputs);

        Ok(format!(
            "[SWARM CONSENSUS]\n{}\n\n[SWARM BEST]\n{}",
            consensus, best
        ))
    }
}
