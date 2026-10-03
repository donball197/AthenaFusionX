use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Selection floor shared by hypothesis selection and belief policy.
/// Hypotheses below this score are not selectable.
pub const SELECT_FLOOR: f32 = 0.15;

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

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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
        // TEMP DIAGNOSTIC: trace every propagation pass without changing behavior.
        eprintln!(
            "[BELIEF TRACE] propagate start nodes={} edges={}",
            self.nodes.len(),
            self.edges.len()
        );

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
                let before = node.belief_score;
                let candidate = (0.5 + d).clamp(0.0, 1.0);
                // Keep a fraction of the previous score (PageRank-style damping)
                node.belief_score = DAMPING * candidate + (1.0 - DAMPING) * node.belief_score;
                // E (belief floor): the 0.15 selection floor must hold at the
                // graph boundary. propagate_beliefs is the one chokepoint that
                // can drop a selectable hypothesis below the floor (a single
                // 0.40 Contradicts edge: 0.15 -> 0.1075 -> fixed point 0.10),
                // and the failure paths call it without going through
                // floor_hypothesis_belief_after_failure. Clamp only hypotheses
                // that were selectable BEFORE this pass: an explicit demotion
                // to 0.05 (e.g. HALT-VETO recovery) must stay dead.
                if matches!(node.kind, NodeKind::Hypothesis)
                    && before >= 0.15
                    && node.belief_score < 0.15
                {
                    node.belief_score = 0.15;
                }

                if matches!(node.kind, NodeKind::Hypothesis) {
                    eprintln!(
                        "[BELIEF TRACE] hypothesis={} before={:.4} delta={:.4} after={:.4}",
                        id, before, d, node.belief_score
                    );
                }
            }
        }

        eprintln!("[BELIEF TRACE] propagate end");
    }

    pub fn select_best_hypothesis(&self) -> Option<&CognitiveNode> {
        self.nodes
            .values()
            // ⚡ FIX: Filter out dead hypotheses (belief < 0.15).
            // v1.9: When BeliefVector is available on the node, prefer may_prove() over raw score.
            .filter(|n| matches!(n.kind, NodeKind::Hypothesis) && n.belief_score >= SELECT_FLOOR)
            .max_by(|a, b| {
                let score_a = a.belief_score / (a.computation_cost as f32).max(1.0);
                let score_b = b.belief_score / (b.computation_cost as f32).max(1.0);
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Return the id of the first node whose content exactly matches
    /// `content`.
    pub fn find_by_content(&self, content: &str) -> Option<Uuid> {
        self.nodes
            .iter()
            .find(|(_, node)| node.content == content)
            .map(|(id, _)| *id)
    }

    /// Return node content, or an empty string when the id is unknown.
    pub fn content_of(&self, id: Uuid) -> String {
        self.nodes
            .get(&id)
            .map(|node| node.content.clone())
            .unwrap_or_default()
    }

    /// Return the current belief score for a node.
    pub fn belief_of(&self, id: Uuid) -> Option<f32> {
        self.nodes
            .get(&id)
            .map(|node| node.belief_score)
    }

    /// Demote a hypothesis to an explicit policy-selected score.
    ///
    /// The caller decides WHY the hypothesis should be demoted
    /// (dead symbol, critic veto, strategy violation, ...).
    /// The graph owns the actual belief-state mutation.
    pub fn demote_hypothesis(&mut self, id: Uuid, score: f32) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.belief_score = score;
            true
        } else {
            false
        }
    }

    /// Promote a hypothesis to at least `minimum`.
    pub fn promote_hypothesis(&mut self, id: Uuid, minimum: f32) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            if node.belief_score < minimum {
                node.belief_score = minimum;
            }
            true
        } else {
            false
        }
    }

    /// Cap beliefs for every node whose content matches `pred`.
    /// Bulk filter-and-cap only: the policy for WHICH strategies are
    /// capped stays with the caller.
    pub fn cap_matching(&mut self, pred: impl Fn(&str) -> bool, cap: f32) {
        for node in self.nodes.values_mut() {
            if pred(&node.content) && node.belief_score > cap {
                node.belief_score = cap;
            }
        }
    }

    /// Apply unclamped belief decay.
    ///
    /// This intentionally permits crossing SELECT_FLOOR, which is what
    /// forced-pivot callers intend. The clamped decay helper in
    /// cortical_loop.rs is untouched here; unclamping it is F4 and lands
    /// in Pass C.
    pub fn decay(&mut self, id: Uuid, delta: f32) -> Option<f32> {
        let node = self.nodes.get_mut(&id)?;
        node.belief_score = (node.belief_score - delta).max(0.0);
        Some(node.belief_score)
    }

    /// Restore a hypothesis to `floor` after evaluation failure, but only
    /// if it was already at or above that floor before evaluation: the
    /// floor is not a resurrection mechanism.
    pub fn restore_floor(&mut self, id: Uuid, pre_eval_belief: f32, floor: f32) {
        if pre_eval_belief < floor {
            return;
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            if node.belief_score < floor {
                node.belief_score = floor;
            }
        }
    }

    /// Return the highest belief among hypothesis nodes.
    pub fn best_hypothesis_belief(&self) -> f32 {
        self.nodes
            .values()
            .filter(|node| matches!(node.kind, NodeKind::Hypothesis))
            .map(|node| node.belief_score)
            .fold(0.0_f32, f32::max)
    }

    /// Record a contradicting observation against `target` and propagate
    /// the resulting graph beliefs. Collapses the add_node + add_edge +
    /// propagate_beliefs triple previously typed at seven call sites.
    pub fn observe_contradiction(
        &mut self,
        content: &str,
        target: Uuid,
        cost: u32,
        weight: f32,
    ) -> Uuid {
        let observation_id = self.add_node(
            NodeKind::Observation,
            content,
            1.0,
            cost,
        );

        self.add_edge(
            observation_id,
            target,
            EdgeRelation::Contradicts,
            weight,
        );

        self.propagate_beliefs();

        observation_id
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_contradiction_propagation_converges_without_compounding_edge_weight() {
        let mut graph = CognitiveGraph::new();

        let observation = graph.add_node(NodeKind::Observation, "compiler contradiction", 1.0, 1);
        let hypothesis = graph.add_node(NodeKind::Hypothesis, "candidate repair", 0.8, 1);

        graph.add_edge(observation, hypothesis, EdgeRelation::Contradicts, 0.30);

        graph.propagate_beliefs();
        let first = graph.nodes[&hypothesis].belief_score;

        graph.propagate_beliefs();
        let second = graph.nodes[&hypothesis].belief_score;

        graph.propagate_beliefs();
        let third = graph.nodes[&hypothesis].belief_score;

        // The same persistent edge must not behave like three independent
        // 0.30 contradictions. Repeated propagation approaches the fixed
        // point implied by the existing damped recurrence.
        assert!(first < 0.8);
        assert!(second < first);
        assert!(third < second);

        // Source belief 1.0, contradiction weight 0.30, DAMPING 0.85:
        // fixed point is 0.20. Third step is 0.202025.
        assert!(third > 0.20);
    }
}
