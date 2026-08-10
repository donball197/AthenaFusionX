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

    /// Marks a task as completed, which may unlock downstream dependencies.
    pub fn complete_task(&mut self, id: &str) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.status = TaskStatus::Completed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_dependency_resolution() {
        let mut dag = DagManager::new();

        // Task A & B: No dependencies
        dag.add_task(TaskNode {
            id: "task_a".to_string(),
            description: "Setup Database".to_string(),
            required_tier: "light".to_string(),
            dependencies: vec![],
            status: TaskStatus::Pending,
        });

        dag.add_task(TaskNode {
            id: "task_b".to_string(),
            description: "Initialize Repo".to_string(),
            required_tier: "light".to_string(),
            dependencies: vec![],
            status: TaskStatus::Pending,
        });

        // Task C: Depends on A and B
        dag.add_task(TaskNode {
            id: "task_c".to_string(),
            description: "Build API endpoints".to_string(),
            required_tier: "heavy".to_string(),
            dependencies: vec!["task_a".to_string(), "task_b".to_string()],
            status: TaskStatus::Pending,
        });

        // Task D: Depends on C
        dag.add_task(TaskNode {
            id: "task_d".to_string(),
            description: "Run Integration Tests".to_string(),
            required_tier: "heavy".to_string(),
            dependencies: vec!["task_c".to_string()],
            status: TaskStatus::Pending,
        });

        // 1. Initially, only A and B should be ready
        let mut ready = dag.get_ready_tasks();
        ready.sort(); // Sort for predictable assertion
        assert_eq!(ready, vec!["task_a", "task_b"]);

        // 2. Complete Task A; C should still wait on B
        dag.complete_task("task_a");
        let ready = dag.get_ready_tasks();
        assert_eq!(ready, vec!["task_b"]);

        // 3. Complete Task B; C should now unlock
        dag.complete_task("task_b");
        let ready = dag.get_ready_tasks();
        assert_eq!(ready, vec!["task_c"]);

        // 4. Complete Task C; D should now unlock
        dag.complete_task("task_c");
        let ready = dag.get_ready_tasks();
        assert_eq!(ready, vec!["task_d"]);
    }
}
