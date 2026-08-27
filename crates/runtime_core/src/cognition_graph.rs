use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Observation,
    Verification,
    Reflection,
    Hypothesis,
    ProposedAction { action_json: String },
    Strategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub goal_satisfied: bool,
    pub evidence_confidence: f32,
    pub independent_check_passed: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveNode {
    pub id: Uuid,
    pub kind: NodeKind,
    pub content: String,
    pub belief_score: f32,
    pub computation_cost: u32,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeRelation {
    Supports,
    Contradicts,
    Prerequisite,
    DerivesFrom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: Uuid,
    pub target: Uuid,
    pub relation: EdgeRelation,
    pub weight: f32,
}

#[derive(Debug, Clone, Default)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CognitiveGraph {
    pub nodes: HashMap<Uuid, CognitiveNode>,
    pub edges: Vec<GraphEdge>,
    pub frontiers: HashSet<Uuid>,
}

impl CognitiveGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(
        &mut self,
        kind: NodeKind,
        content: &str,
        initial_belief: f32,
        cost: u32,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let node = CognitiveNode {
            id,
            kind,
            content: content.to_string(),
            belief_score: initial_belief,
            computation_cost: cost,
            is_active: false,
        };
        self.nodes.insert(id, node);
        self.frontiers.insert(id);
        id
    }

    pub fn add_edge(&mut self, source: Uuid, target: Uuid, relation: EdgeRelation, weight: f32) {
        self.edges.push(GraphEdge { source, target, relation, weight });
    }

    pub fn propagate_beliefs(&mut self) {
        // Accumulate signed impacts first so multiple edges targeting the same
        // hypothesis are combined instead of overwriting each other.
        let mut delta: HashMap<Uuid, f32> = HashMap::new();
        const DAMPING: f32 = 0.85;

        for edge in &self.edges {
            let Some(source_node) = self.nodes.get(&edge.source) else {
                continue;
            };
            let impact = source_node.belief_score * edge.weight;
            let signed = match edge.relation {
                EdgeRelation::Supports => impact,
                EdgeRelation::Contradicts => -impact,
                // Prerequisite / DerivesFrom intentionally ignored for score updates
                _ => continue,
            };
            *delta.entry(edge.target).or_insert(0.0) += signed;
        }

        for (id, d) in delta {
            if let Some(node) = self.nodes.get_mut(&id) {
                let candidate = (node.belief_score + d).clamp(0.0, 1.0);
                // Keep a fraction of the previous score (PageRank-style damping)
                node.belief_score = DAMPING * candidate + (1.0 - DAMPING) * node.belief_score;
            }
        }
    }

    pub fn select_best_hypothesis(&self) -> Option<&CognitiveNode> {
        self.nodes
            .values()
            // ⚡ FIX: Filter out dead hypotheses (belief < 0.15).
            // v1.9: When BeliefVector is available on the node, prefer may_prove() over raw score.
            .filter(|n| matches!(n.kind, NodeKind::Hypothesis) && n.belief_score >= 0.15)
            .max_by(|a, b| {
                let score_a = a.belief_score / (a.computation_cost as f32).max(1.0);
                let score_b = b.belief_score / (b.computation_cost as f32).max(1.0);
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
