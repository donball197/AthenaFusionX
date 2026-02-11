use crate::tflite_bridge;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;

pub struct NativeAgent {
    pub id: String,
    pub model_path: String,
    pub is_active: bool,
}

pub struct NativeAgentManager {
    pub agents: Arc<Mutex<HashMap<String, NativeAgent>>>,
    pub max_active_slots: usize,
}

impl NativeAgentManager {
    pub fn init() -> Self {
        let mut agents = HashMap::new();
        // Registering the 68-agent swarm in cold storage
        for i in 0..68 {
            let id = format!("agent_{}", i);
            agents.insert(id.clone(), NativeAgent {
                id: id.clone(),
                model_path: format!("./models/{}.tflite", id),
                is_active: false,
            });
        }
        
        Self {
            agents: Arc::new(Mutex::new(agents)),
            max_active_slots: 4, // Strict throttle for Motorola G RAM safety
        }
    }

    pub async fn ignite_agent(&self, agent_id: &str) -> Result<()> {
        let mut agents = self.agents.lock().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            println!(">> [SWARM] Activating Agent: {}", agent_id);
            tflite_bridge::ignite_agent(&agent.model_path)?;
            agent.is_active = true;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Agent {} not found in registry", agent_id))
        }
    }
}
