use std::collections::{HashMap, VecDeque};
use crate::orchestrator_v2::task_graph::TaskGraph;

pub fn validate_graph(graph: &TaskGraph) -> Result<(), String> {
    let mut indegree = HashMap::new();

    for (id, node) in &graph.nodes {
        indegree.entry(id.clone()).or_insert(0);
        for dep in &node.deps {
            *indegree.entry(dep.clone()).or_insert(0) += 1;
        }
    }

    let mut queue = VecDeque::new();
    for (node, degree) in &indegree {
        if *degree == 0 { queue.push_back(node.clone()); }
    }

    let mut visited = 0;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        if let Some(n) = graph.nodes.get(&node) {
            for dep in &n.deps {
                if let Some(v) = indegree.get_mut(dep) {
                    *v -= 1;
                    if *v == 0 { queue.push_back(dep.clone()); }
                }
            }
        }
    }

    if visited != graph.nodes.len() {
        return Err("GRAPH_CYCLE_DETECTED".into());
    }
    Ok(())
}
