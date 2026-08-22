use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompletionCondition {
    ReadMarkerAcquired,
    SearchMarkerAcquired,
    WriteMarkerAcquired,
    PatchMarkerAcquired,
    CommandMarkerAcquired,
    SourceCodeEvidenceRequired,
    MinBeliefScore(f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgoalContract {
    pub node_id: String,
    pub objective: String,
    pub conditions: Vec<CompletionCondition>,
    pub is_mutation_task: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveBudget {
    pub max_turns: u32,
    pub max_model_calls: u32,
    pub model_calls_made: u32,
}

impl CognitiveBudget {
    pub fn default_for_node() -> Self {
        Self { max_turns: 60, max_model_calls: 150, model_calls_made: 0 }
    }

    pub fn record_call(&mut self) {
        self.model_calls_made += 1;
    }

    pub fn is_exhausted(&self, current_turn: u32) -> bool {
        current_turn >= self.max_turns || self.model_calls_made >= self.max_model_calls
    }
}

pub struct SubgoalCompletionEngine;

fn has_word(text: &str, word: &str) -> bool {
    text.split_whitespace()
        .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()) == word)
}

fn any_word(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| has_word(text, w))
}

fn has_word_unnegated(text: &str, word: &str) -> bool {
    let negations = ["not", "never", "no", "cannot", "n't"];
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (i, tok) in tokens.iter().enumerate() {
        let clean = tok.trim_matches(|c: char| !c.is_alphanumeric());
        if clean == word {
            let start = i.saturating_sub(3);
            let negated = tokens[start..i].iter().any(|w| {
                let wl = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                negations.contains(&wl.as_str()) || wl.ends_with("n't")
            });
            if !negated {
                return true;
            }
        }
    }
    false
}

fn any_word_unnegated(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| has_word_unnegated(text, w))
}

impl SubgoalCompletionEngine {
    pub fn infer_contract(node_id: &str, objective: &str) -> SubgoalContract {
        let obj_lower = objective.trim().to_lowercase();
        let classify_text = obj_lower
            .strip_prefix("objective:")
            .or_else(|| obj_lower.strip_prefix("objective"))
            .unwrap_or(&obj_lower)
            .trim();

        let mut conditions = Vec::new();
        let mut is_mutation_task = false;

        let has_patch = any_word_unnegated(classify_text, &["patch", "apply", "fix", "remediate", "mutate"]);
        let has_write = any_word_unnegated(classify_text, &["write", "export", "save", "implement", "integrate", "add", "create"]);

        if has_patch || has_write {
            is_mutation_task = true;
            if has_patch {
                conditions.push(CompletionCondition::PatchMarkerAcquired);
            } else {
                conditions.push(CompletionCondition::WriteMarkerAcquired);
            }
            if any_word(classify_text, &["roadmap", "audit", "comparison"]) {
                conditions.push(CompletionCondition::SourceCodeEvidenceRequired);
            }
        } else {
            let first_word = classify_text.split_whitespace().next().unwrap_or("");
            let is_recon_intent = [
                "read", "analyze", "inspect", "search", "examine", "locate", "profile", "benchmark", "audit"
            ].contains(&first_word);

            if is_recon_intent || any_word(classify_text, &["read", "inspect", "audit", "search", "find", "locate"]) {
                if any_word(classify_text, &["search", "find", "locate"]) {
                    conditions.push(CompletionCondition::SearchMarkerAcquired);
                } else {
                    conditions.push(CompletionCondition::ReadMarkerAcquired);
                }
                conditions.push(CompletionCondition::MinBeliefScore(0.65));
            } else {
                conditions.push(CompletionCondition::CommandMarkerAcquired);
                conditions.push(CompletionCondition::MinBeliefScore(0.85));
            }
        }

        SubgoalContract {
            node_id: node_id.to_string(),
            objective: objective.to_string(),
            conditions,
            is_mutation_task,
        }
    }

    pub fn evaluate(
        contract: &SubgoalContract, 
        local_evidence: &str, 
        belief_score: f32
    ) -> bool {
        // ⚡ DETERMINISTIC VERIFIER INTERFACE
        // Passes the contract conditions and belief score to the strict verifier
        let verifier_result = crate::outcome_verifier::OutcomeVerifier::verify(
            &contract.node_id, 
            local_evidence,
            belief_score,
            &contract.conditions
        );
        matches!(verifier_result, crate::outcome_verifier::CompletionStatus::Approved)
    }
}
