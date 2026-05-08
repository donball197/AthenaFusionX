use std::collections::{HashMap, HashSet, VecDeque};
use anyhow::{anyhow, Result};

#[derive(Clone)]
pub struct TaskNode {
    pub id:             String,
    pub deps:           Vec<String>,
    pub required_agent: String,
}

pub struct TaskGraph { pub nodes: HashMap<String, TaskNode> }

impl TaskGraph {
    pub fn new() -> Self { Self { nodes: HashMap::new() } }
    pub fn add_task(&mut self, id: &str, deps: Vec<String>, required_agent: &str) {
        self.nodes.insert(id.to_string(), TaskNode { id: id.to_string(), deps, required_agent: required_agent.to_string() });
    }

    pub fn resolve_execution_order(&self) -> Result<Vec<String>> {
        for node in self.nodes.values() {
            for dep in &node.deps {
                if !self.nodes.contains_key(dep.as_str()) { return Err(anyhow!("DAG_ERROR: Task '{}' depends on unknown task '{}'.", node.id, dep)); }
            }
        }

        let mut in_degree: HashMap<&str, usize> = self.nodes.keys().map(|k| (k.as_str(), 0)).collect();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for node in self.nodes.values() {
            for dep in &node.deps {
                dependents.entry(dep.as_str()).or_default().push(node.id.as_str());
                *in_degree.entry(node.id.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();
        let mut order = Vec::with_capacity(self.nodes.len());

        while let Some(id) = queue.pop_front() {
            order.push(id.to_string());
            if let Some(kids) = dependents.get(id) {
                for &child in kids {
                    let deg = in_degree.get_mut(child).unwrap();
                    *deg -= 1;
                    if *deg == 0 { queue.push_back(child); }
                }
            }
        }

        if order.len() != self.nodes.len() {
            let resolved: HashSet<_> = order.iter().map(|s| s.as_str()).collect();
            let cycle_nodes: Vec<_> = self.nodes.keys().filter(|k| !resolved.contains(k.as_str())).cloned().collect();
            return Err(anyhow!("DAG_DEADLOCK: Circular dependency detected among: {:?}", cycle_nodes));
        }
        Ok(order)
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}
