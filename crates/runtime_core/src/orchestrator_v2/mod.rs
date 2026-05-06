pub mod task_graph;

use crate::agent_runtime::engine::{Task, TaskResult, ExecutableAgent};
use self::task_graph::TaskGraph;
use std::collections::HashMap;
use anyhow::{anyhow, Result};

pub struct OrchestratorV2 {
    // ⚡ DYNAMIC DISPATCH: Supports a diverse swarm of agents simultaneously
    agents: HashMap<String, Box<dyn ExecutableAgent>>, 
}

impl OrchestratorV2 {
    pub fn new() -> Self {
        Self { agents: HashMap::new() }
    }

    pub fn register_agent(&mut self, name: &str, agent: Box<dyn ExecutableAgent>) {
        self.agents.insert(name.to_string(), agent);
    }

    pub async fn execute_graph(&self, graph: TaskGraph) -> Result<Vec<TaskResult>> {
        let order = graph.resolve_execution_order()?;
        let mut results = Vec::new();

        for id in order {
            let node = graph.nodes.get(&id).unwrap();
            
            let agent = self.agents.get(&node.required_agent)
                .ok_or_else(|| anyhow!("AGENT_MISSING: '{}' not registered in swarm.", node.required_agent))?;

            let task = Task {
                id: id.clone(),
                payload: format!("Execute directive for {}", id),
            };

            let result = agent.execute(task).await?;
            results.push(result);
        }

        Ok(results)
    }
}
