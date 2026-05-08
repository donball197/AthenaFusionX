use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct ExecutionNode {
    pub id: String,
    pub deps: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ExecutionGraph {
    pub nodes: HashMap<String, ExecutionNode>,
}

impl ExecutionGraph {
    pub fn validate_and_sort(&self) -> Result<Vec<String>, String> {
        let mut indegree: HashMap<String, usize> = HashMap::new();
        let mut sorted = Vec::new();

        // Initialize indegrees
        for (id, node) in &self.nodes {
            indegree.entry(id.clone()).or_insert(0);
            for dep in &node.deps {
                *indegree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        let mut queue = VecDeque::new();
        for (node, degree) in &indegree {
            if *degree == 0 {
                queue.push_back(node.clone());
            }
        }

        let mut visited = 0;
        while let Some(node) = queue.pop_front() {
            visited += 1;
            sorted.push(node.clone());

            if let Some(n) = self.nodes.get(&node) {
                for dep in &n.deps {
                    if let Some(v) = indegree.get_mut(dep) {
                        *v -= 1;
                        if *v == 0 {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }

        if visited != self.nodes.len() {
            return Err("FRACTURE: Cyclic dependency detected in DAG. Execution aborted.".into());
        }

        // Reverse to get execution order (dependencies must run first)
        sorted.reverse();
        Ok(sorted)
    }
}
