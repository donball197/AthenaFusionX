fn distill_to_proposition(raw: &str) -> (String, Option<EpistemicTuple>) {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.is_empty() {
        return (raw.to_string(), None);
    }

    let first = lines[0].trim();

    // 1. Distill [READ_OK]
    if first.starts_with("[READ_OK]") || first.starts_with("[READ_CHUNK_OK]") {
        let path = first.split_whitespace().nth(1).unwrap_or("unknown_file");
        let mut key_symbols = Vec::new();
        for l in lines.iter().skip(1) {
            let s = l.trim();
            if s.starts_with("pub struct ")
                || s.starts_with("pub enum ")
                || s.starts_with("pub fn ")
                || s.starts_with("pub trait ")
            {
                let sig = s.split('{').next().unwrap_or(s).trim();
                key_symbols.push(sig);
            }
        }
        let symbols_summary = if key_symbols.is_empty() {
            format!("File read ({} lines)", lines.len())
        } else {
            key_symbols.join(", ")
        };

        let tuple = EpistemicTuple::new("AST", path, "DEFINES", &symbols_summary, 1.0);
        let prop =
            format!("{}\n[PROPOSITION] {} -> Defined symbols: {}", first, path, symbols_summary);
        return (prop, Some(tuple));
    }

    // 2. Distill [SEARCH_CODE_OK]
    if first.starts_with("[SEARCH_CODE_OK]") {
        let is_empty = raw.contains("No code matches found");
        let path = raw.split("Path:").nth(1).and_then(|s| s.split('|').next()).unwrap_or("").trim();
        let query = raw
            .split("Query:")
            .nth(1)
            .and_then(|s| s.split('\n').next())
            .unwrap_or("")
            .trim()
            .trim_matches('\'');

        if is_empty {
            let tuple = EpistemicTuple::new("AST", path, "NOT_FOUND", query, 1.0);
            let prop = format!(
                "{}\n[PROPOSITION] Refuted: symbol '{}' does not exist in '{}'",
                first, query, path
            );
            return (prop, Some(tuple));
        } else {
            let matches: Vec<&str> = lines
                .iter()
                .filter(|l| l.contains(':') && !l.starts_with('['))
                .take(5)
                .copied()
                .collect();
            let summary = matches.join("; ");
            let tuple = EpistemicTuple::new("AST", path, "MATCHES", &summary, 1.0);
            let prop =
                format!("{}\n[PROPOSITION] Verified: matches in '{}' -> {}", first, path, summary);
            return (prop, Some(tuple));
        }
    }

    // 3. Distill semantic-memory recall.
    //
    // Memory retrieval is evidence about prior experience, not proof that
    // the current objective is complete. Keep successful recall and empty
    // recall explicitly distinguishable so cognition can use both outcomes.
    if first.starts_with("[SEARCH_MEMORY_OK]") {
        let summary = lines
            .iter()
            .skip(1)
            .take(8)
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let value = if summary.is_empty() {
            "Semantic memory returned recalled context.".to_string()
        } else {
            summary
        };

        let tuple =
            EpistemicTuple::new("MEMORY", "semantic_recall", "SUCCESS", &value, 1.0);

        let prop = format!(
            "{}\n[PROPOSITION] Recalled prior experience: {}",
            first, value
        );

        return (prop, Some(tuple));
    }

    if first.starts_with("[SEARCH_MEMORY_EMPTY]") {
        let tuple =
            EpistemicTuple::new("MEMORY", "semantic_recall", "EMPTY", first, 1.0);

        let prop = format!(
            "{}\n[PROPOSITION] No relevant prior memory was retrieved.",
            first
        );

        return (prop, Some(tuple));
    }

    // 4. Distill [CMD_OK] / [COMPILE_OK]
    if first.starts_with("[CMD_OK]") || first.starts_with("[COMPILE_OK]") {
        let tuple = EpistemicTuple::new("COMMAND", "status", "SUCCESS", first, 1.0);
        return (first.to_string(), Some(tuple));
    }

    // 5. Distill [ACTION FAILED]
    if first.starts_with("[ACTION FAILED]") {
        let err = lines.get(1).unwrap_or(&first);
        let tuple = EpistemicTuple::new("OS", "action", "FAILED", err, 0.9);
        let prop = format!("{}\n[PROPOSITION] Action failed: {}", first, err);
        return (prop, Some(tuple));
    }

    (raw.to_string(), None)
}

use crate::cognition_graph::{CognitiveGraph, EdgeRelation, NodeKind};
use crate::cognitive_events::CognitiveEvent;
use crate::completion_engine::{CognitiveBudget, SubgoalCompletionEngine, SubgoalContract};
use crate::epistemic_engine::CriticResponse;
use crate::epistemic_tuple::EpistemicTuple;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpistemicSession {
    pub session_id: Uuid,
    pub mission: String,
    pub contract: SubgoalContract,
    pub budget: CognitiveBudget,
    pub graph: CognitiveGraph,
    pub local_evidence: String,
    pub executed_actions: HashMap<String, u32>,
    #[serde(skip)]
    pub global_action_attempts: HashMap<String, u32>,
    pub max_observed_belief: f32,
    pub best_hypothesis_id: Option<Uuid>,
    pub consecutive_mutation_vetoes: u32,
    pub tuples: Vec<EpistemicTuple>,
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
            executed_actions: HashMap::new(),
            global_action_attempts: HashMap::new(),
            max_observed_belief: 0.0,
            best_hypothesis_id: None,
            consecutive_mutation_vetoes: 0,
            tuples: Vec::new(),
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
                let (distilled_prop, maybe_tuple) = distill_to_proposition(evidence);
                if let Some(t) = maybe_tuple {
                    self.tuples.push(t);
                }

                self.local_evidence.push_str(&distilled_prop);
                self.local_evidence.push('\n');

                if let Some(h_id) = self.best_hypothesis_id {
                    let obs_id =
                        self.graph.add_node(NodeKind::Observation, &distilled_prop, 1.0, 50);
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
        let count = self.executed_actions.entry(action_key.to_string()).or_insert(0);
        *count += 1;
    }

    pub fn action_attempt_count(&self, action_key: &str) -> Option<u32> {
        self.executed_actions.get(action_key).copied()
    }

    pub fn is_action_attempted(&self, action_key: &str) -> bool {
        self.action_attempt_count(action_key).map(|c| c > 0).unwrap_or(false)
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
        // Belief ranks hypotheses; physical contract evidence authorizes
        // completion. Never let an epistemic confidence threshold by itself
        // terminate a subgoal.
        //
        // The completion engine still receives the belief value for legacy
        // compatibility. Its MinBeliefScore condition is inspected separately
        // before this becomes the authoritative completion path.
        SubgoalCompletionEngine::evaluate(
            &self.contract,
            &self.local_evidence,
            current_belief,
        )
    }

    /// Preserve physical session evidence when cognition terminates
    /// without a currently selectable hypothesis.
    ///
    /// Terminal failure is an epistemic/control-flow outcome. It must
    /// never erase physical evidence already acquired by execution.
    pub fn terminal_evidence(&self, reason: &str) -> String {
        let physical = self.local_evidence.trim();

        if physical.is_empty() {
            return reason.to_string();
        }

        format!(
            "{}\n\n[PHYSICAL SESSION EVIDENCE]\n{}",
            reason, physical
        )
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

    pub fn record_global_action_attempt(&mut self, action_key: &str) {
        *self.global_action_attempts.entry(action_key.to_string()).or_insert(0) += 1;
    }

    pub fn global_action_attempt_count(&self, action_key: &str) -> u32 {
        self.global_action_attempts.get(action_key).copied().unwrap_or(0)
    }
}
