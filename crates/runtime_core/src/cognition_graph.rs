use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Observation, Hypothesis, ProposedAction { action_json: String }, Strategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveNode {
    pub id: Uuid, pub kind: NodeKind, pub content: String,
    pub belief_score: f32, pub computation_cost: u32, pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeRelation { Supports, Contradicts, Prerequisite, DerivesFrom }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: Uuid, pub target: Uuid, pub relation: EdgeRelation, pub weight: f32,
}

#[derive(Debug, Clone, Default)]
pub struct CognitiveGraph {
    pub nodes: HashMap<Uuid, CognitiveNode>,
    pub edges: Vec<GraphEdge>,
    pub frontiers: HashSet<Uuid>,
}

impl CognitiveGraph {
    pub fn new() -> Self { Self::default() }

    pub fn add_node(&mut self, kind: NodeKind, content: &str, initial_belief: f32, cost: u32) -> Uuid {
        let id = Uuid::new_v4();
        let node = CognitiveNode { id, kind, content: content.to_string(), belief_score: initial_belief, computation_cost: cost, is_active: false };
        self.nodes.insert(id, node);
        self.frontiers.insert(id);
        id
    }

    pub fn add_edge(&mut self, source: Uuid, target: Uuid, relation: EdgeRelation, weight: f32) {
        self.edges.push(GraphEdge { source, target, relation, weight });
    }

    pub fn propagate_beliefs(&mut self) {
        let mut updates: HashMap<Uuid, f32> = HashMap::new();
        for edge in &self.edges {
            if let Some(source_node) = self.nodes.get(&edge.source) {
                if let Some(target_node) = self.nodes.get(&edge.target) {
                    let impact = source_node.belief_score * edge.weight;
                    let mut new_belief = target_node.belief_score;
                    match edge.relation {
                        EdgeRelation::Supports => new_belief = (new_belief + impact).min(1.0),
                        EdgeRelation::Contradicts => new_belief = (new_belief - impact).max(0.0),
                        _ => {}
                    }
                    updates.insert(edge.target, new_belief);
                }
            }
        }
        for (id, new_score) in updates {
            if let Some(node) = self.nodes.get_mut(&id) { node.belief_score = new_score; }
        }
    }

    pub fn select_best_hypothesis(&self) -> Option<&CognitiveNode> {
        self.nodes.values()
            // ⚡ FIX: Filter out dead hypotheses (belief < 0.15) so we don't get stuck in a death loop!
            .filter(|n| matches!(n.kind, NodeKind::Hypothesis) && n.belief_score >= 0.15)
            .max_by(|a, b| {
                let score_a = a.belief_score / (a.computation_cost as f32).max(1.0);
                let score_b = b.belief_score / (b.computation_cost as f32).max(1.0);
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
