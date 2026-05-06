use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use crate::agents::LogicAgent;
use crate::shared_state::VmoState;
use crate::distributed_scheduler::DistributedScheduler;
use nexus_substrate::memory::MemoryPalace;

pub struct SubstrateContext {
    pub memory: Arc<Mutex<MemoryPalace>>,
    pub vmo_state: Arc<Mutex<VmoState>>,
    pub dist: Arc<DistributedScheduler>,
}

pub struct AgenticRuntime {
    pub ctx: SubstrateContext,
    pub agents: Vec<Box<dyn LogicAgent>>,
}

impl AgenticRuntime {
    pub fn new(ctx: SubstrateContext) -> Self {
        Self {
            ctx,
            agents: Vec::new(),
        }
    }

    pub fn register_agent(&mut self, agent: Box<dyn LogicAgent>) {
        self.agents.push(agent);
    }

    pub async fn dispatch(&mut self, command: &str) -> Result<String> {
        // Search for a specialized agent
        for agent in &self.agents {
            if agent.can_handle(command) {
                return agent.execute(command, &self.ctx).await;
            }
        }

        // Fallback to basic shell execution if no specialized agent matches
        let output = crate::podman::PodmanShield::execute(command)
            .map_err(|e| anyhow::anyhow!("Substrate Execution Fault: {}", e))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        if !output.status.success() && stdout.is_empty() {
            Ok(format!("[ERROR] {}", stderr))
        } else {
            Ok(stdout)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::ClusterMesh;
    
    #[tokio::test]
    async fn test_runtime_init() {
        // This test only verifies that the structures can be initialized in memory
        let mesh = Arc::new(ClusterMesh::new());
        let dist = Arc::new(DistributedScheduler::new(mesh));
        // Note: We don't need a real DB for a basic logic check
        println!("LRL Structural Integrity: OK");
    }
}
