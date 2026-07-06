use crate::orchestrator_v2::task_graph::TaskGraph;

pub struct CriticAgent;

impl CriticAgent {
    pub fn evaluate(graph: &TaskGraph) -> f32 {
        if graph.nodes.is_empty() { return 0.0; }
        let mut score = 1.0_f32;
        for node in graph.nodes.values() {
            if node.deps.is_empty() && graph.nodes.len() > 1 { score *= 0.9; }
            if node.required_agent.trim().is_empty() { score *= 0.6; }
            if node.priority == 0 { score *= 0.85; }
        }
        score.clamp(0.0, 1.0)
    }
}


