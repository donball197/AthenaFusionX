use std::collections::{HashMap, VecDeque};
use anyhow::{anyhow, Result};

#[derive(Clone)]
pub struct TaskNode {
    pub id: String,
    pub deps: Vec<String>,
    pub required_agent: String,
}

pub struct TaskGraph {
    pub nodes: HashMap<String, TaskNode>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    pub fn add_task(&mut self, id: &str, deps: Vec<String>, required_agent: &str) {
        self.nodes.insert(id.to_string(), TaskNode {
            id: id.to_string(),
            deps,
            required_agent: required_agent.to_string(),
        });
    }

    pub fn resolve_execution_order(&self) -> Result<Vec<String>> {
        let mut resolved = Vec::new();
        let mut queue: VecDeque<_> = self.nodes.values().cloned().collect();
        
        let mut iterations = 0;
        let max_iterations = self.nodes.len() * self.nodes.len() + 1;

        while let Some(node) = queue.pop_front() {
            iterations += 1;
            
            // ⚡ DEADLOCK TRAP FIXED
            if iterations > max_iterations {
                return Err(anyhow!("DAG_DEADLOCK: Circular dependency detected. Aborting."));
            }

            if node.deps.iter().all(|d| resolved.contains(d)) {
                resolved.push(node.id.clone());
            } else {
                queue.push_back(node);
            }
        }
        Ok(resolved)
    }
}
