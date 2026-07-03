use anyhow::{anyhow, Result};
use std::collections::{HashMap, VecDeque};
#[allow(unused_imports)]
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    Completed,
    Ready,
    Escalated,
    Retrying,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TaskExecutionState {
    pub attempts: u8,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    
    #[serde(alias = "dependencies")]
    pub deps: Vec<String>,
    
    #[serde(alias = "assigned_agent")]
    pub required_agent: String,

    #[serde(default = "default_status")]
    pub status: TaskStatus,

    #[serde(default)]
    pub objective: String,

    #[serde(default)]
    pub success_criteria: Vec<String>,

    #[serde(default)]
    pub execution_state: TaskExecutionState,

    // HEFT Optimization field
    #[serde(default)]
    pub priority: usize,
}

fn default_status() -> TaskStatus {
    TaskStatus::Pending
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskGraph {
    pub nodes: HashMap<String, TaskNode>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }
    
    pub fn add_task(&mut self, id: &str, deps: Vec<String>, required_agent: &str) {
        self.nodes.insert(
            id.to_string(),
            TaskNode { 
                id: id.to_string(), 
                deps, 
                required_agent: required_agent.to_string(),
                status: TaskStatus::Pending,
                objective: String::new(),
                success_criteria: Vec::new(),
                execution_state: TaskExecutionState::default(),
                priority: 0,
            },
        );
    }

    pub fn compute_critical_path_priorities(&mut self) {
        let mut memo: HashMap<String, usize> = HashMap::new();

        fn b_level(node_id: &str, graph: &TaskGraph, memo: &mut HashMap<String, usize>) -> usize {
            if let Some(&cached) = memo.get(node_id) {
                return cached;
            }
            let mut max_child = 0;
            
            for (child_id, child) in &graph.nodes {
                if child.deps.contains(&node_id.to_string()) {
                    max_child = max_child.max(b_level(child_id, graph, memo));
                }
            }
            let level = max_child + 1;
            memo.insert(node_id.to_string(), level);
            level
        }

        let keys: Vec<String> = self.nodes.keys().cloned().collect();
        for id in keys {
            let prio = b_level(&id, self, &mut memo);
            if let Some(node) = self.nodes.get_mut(&id) {
                node.priority = prio;
            }
        }
    }

    pub fn resolve_execution_order(&self) -> Result<Vec<String>> {
        for node in self.nodes.values() {
            for dep in &node.deps {
                if !self.nodes.contains_key(dep.as_str()) {
                    return Err(anyhow!(
                        "DAG_ERROR: Task '{}' depends on unknown task '{}'.",
                        node.id,
                        dep
                    ));
                }
            }
        }

        let mut in_degree: HashMap<&str, usize> =
            self.nodes.keys().map(|k| (k.as_str(), 0)).collect();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for node in self.nodes.values() {
            for dep in &node.deps {
                dependents.entry(dep.as_str()).or_default().push(node.id.as_str());
                *in_degree.entry(node.id.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> =
            in_degree.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();
        let mut order = Vec::with_capacity(self.nodes.len());

        while let Some(id) = queue.pop_front() {
            order.push(id.to_string());
            if let Some(kids) = dependents.get(id) {
                for &child in kids {
                    let deg = in_degree.get_mut(child).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(child);
                    }
                }
            }
        }

        if order.len() != self.nodes.len() {
            let resolved: HashSet<_> = order.iter().map(|s| s.as_str()).collect();
            let cycle_nodes: Vec<_> =
                self.nodes.keys().filter(|k| !resolved.contains(k.as_str())).cloned().collect();
            return Err(anyhow!(
                "DAG_DEADLOCK: Circular dependency detected among: {:?}",
                cycle_nodes
            ));
        }
        Ok(order)
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}
