use crate::cognition_graph::{CognitiveGraph, EdgeRelation, NodeKind};
use crate::cognitive_events::CognitiveEvent;
use crate::completion_engine::{CognitiveBudget, SubgoalCompletionEngine, SubgoalContract};
use crate::epistemic_engine::CriticResponse;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EpistemicSession {
    pub session_id: Uuid,
    pub mission: String,
    pub contract: SubgoalContract,
    pub budget: CognitiveBudget,
    pub graph: CognitiveGraph,
    pub local_evidence: String,
    pub executed_actions: HashSet<String>,
    pub max_observed_belief: f32,
    pub best_hypothesis_id: Option<Uuid>,
    pub consecutive_mutation_vetoes: u32,
}

impl EpistemicSession {
    pub fn new(mission: &str) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            mission: mission.to_string(),
            contract: SubgoalCompletionEngine::infer_contract("session_node", mission),
            budget: CognitiveBudget::default_for_node(),
            graph: CognitiveGraph::new(),
            local_evidence: String::new(),
            executed_actions: HashSet::new(),
            max_observed_belief: 0.0,
            best_hypothesis_id: None,
            consecutive_mutation_vetoes: 0,
        }
    }

    pub fn apply_event(&mut self, event: &CognitiveEvent) {
        match event {
            CognitiveEvent::HypothesisCreated { hypothesis_id, hypothesis, .. } => {
                self.graph.add_node(NodeKind::Hypothesis, hypothesis, 0.5, 100);
                if self.best_hypothesis_id.is_none() {
                    self.best_hypothesis_id = Some(*hypothesis_id);
                }
            }
            CognitiveEvent::ObservationRecorded { evidence, modifier, .. } => {
                self.local_evidence.push_str(evidence);
                self.local_evidence.push('\n');

                if let Some(h_id) = self.best_hypothesis_id {
                    let obs_id = self.graph.add_node(NodeKind::Observation, evidence, 1.0, 50);
                    let relation = if *modifier > 0.0 {
                        EdgeRelation::Supports
                    } else {
                        EdgeRelation::Contradicts
                    };
                    self.graph.add_edge(obs_id, h_id, relation, modifier.abs());
                    self.graph.propagate_beliefs();

                    if let Some(node) = self.graph.nodes.get(&h_id) {
                        if node.belief_score > self.max_observed_belief {
                            self.max_observed_belief = node.belief_score;
                        }
                    }
                }
            }
            CognitiveEvent::BeliefUpdated { hypothesis_id, new_belief, .. } => {
                if let Some(node) = self.graph.nodes.get_mut(hypothesis_id) {
                    node.belief_score = *new_belief;
                    if *new_belief > self.max_observed_belief {
                        self.max_observed_belief = *new_belief;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn record_action_attempt(&mut self, action_key: &str) {
        self.executed_actions.insert(action_key.to_string());
    }

    pub fn is_action_attempted(&self, action_key: &str) -> bool {
        self.executed_actions.contains(action_key)
    }

    pub fn record_evidence(&mut self, evidence: &str, eval: &CriticResponse, h_id: Uuid) {
        let event = CognitiveEvent::ObservationRecorded {
            session_id: self.session_id,
            evidence: evidence.to_string(),
            modifier: eval.confidence_modifier,
        };
        self.best_hypothesis_id = Some(h_id);
        self.apply_event(&event);
    }

    pub fn is_subgoal_satisfied(&self, current_belief: f32) -> bool {
        if current_belief < 0.85 {
            return false;
        }
        SubgoalCompletionEngine::evaluate(&self.contract, &self.local_evidence, current_belief)
    }

    pub fn collect_evidence(&self, h_id: Uuid) -> String {
        let mut history = String::new();
        for edge in &self.graph.edges {
            if edge.target == h_id
                && (edge.relation == EdgeRelation::Supports
                    || edge.relation == EdgeRelation::Contradicts)
            {
                if let Some(obs) = self.graph.nodes.get(&edge.source) {
                    history.push_str(&format!("- [{:?}] {}\n", edge.relation, obs.content));
                }
            }
        }
        if history.is_empty() {
            "No evidence recorded.".to_string()
        } else {
            history
        }
    }
}
