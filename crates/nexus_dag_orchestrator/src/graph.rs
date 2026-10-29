use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: String,
    pub description: String,
    pub required_tier: String, // "light" or "heavy"
    pub dependencies: Vec<String>,
    pub status: TaskStatus,
    pub result_evidence: String, // NEW: Stores the output context of the task
}

pub struct DagManager {
    pub nodes: HashMap<String, TaskNode>,
}

impl DagManager {
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    /// Adds a new task to the execution graph.
    pub fn add_task(&mut self, node: TaskNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Returns a list of Task IDs that have all dependencies met and are ready to run.
    pub fn get_ready_tasks(&self) -> Vec<String> {
        let mut ready = Vec::new();

        for (id, node) in &self.nodes {
            if node.status != TaskStatus::Pending {
                continue;
            }

            let dependencies_met = node.dependencies.iter().all(|dep_id| {
                if let Some(dep_node) = self.nodes.get(dep_id) {
                    dep_node.status == TaskStatus::Completed
                } else {
                    false // Dependency doesn't exist
                }
            });

            if dependencies_met {
                ready.push(id.clone());
            }
        }

        ready
    }

    /// Marks a task as completed and stores its output for downstream nodes.
    pub fn complete_task(&mut self, id: &str, output: String) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.status = TaskStatus::Completed;
            node.result_evidence = output;
        }
    }
}

impl Default for DagManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_dependency_resolution() {
        let mut dag = DagManager::new();

        dag.add_task(TaskNode {
            id: "task_a".to_string(),
            description: "Setup Database".to_string(),
            required_tier: "light".to_string(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            result_evidence: String::new(),
        });

        dag.add_task(TaskNode {
            id: "task_b".to_string(),
            description: "Initialize Repo".to_string(),
            required_tier: "light".to_string(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            result_evidence: String::new(),
        });

        dag.add_task(TaskNode {
            id: "task_c".to_string(),
            description: "Build API endpoints".to_string(),
            required_tier: "heavy".to_string(),
            dependencies: vec!["task_a".to_string(), "task_b".to_string()],
            status: TaskStatus::Pending,
            result_evidence: String::new(),
        });

        dag.add_task(TaskNode {
            id: "task_d".to_string(),
            description: "Run Integration Tests".to_string(),
            required_tier: "heavy".to_string(),
            dependencies: vec!["task_c".to_string()],
            status: TaskStatus::Pending,
            result_evidence: String::new(),
        });

        let mut ready = dag.get_ready_tasks();
        ready.sort();
        assert_eq!(ready, vec!["task_a", "task_b"]);

        dag.complete_task("task_a", "Ok".into());
        let ready = dag.get_ready_tasks();
        assert_eq!(ready, vec!["task_b"]);

        dag.complete_task("task_b", "Ok".into());
        let ready = dag.get_ready_tasks();
        assert_eq!(ready, vec!["task_c"]);

        dag.complete_task("task_c", "Ok".into());
        let ready = dag.get_ready_tasks();
        assert_eq!(ready, vec!["task_d"]);
    }
}
