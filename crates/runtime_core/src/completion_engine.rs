#![allow(clippy::collapsible_match)]
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
    text.split_whitespace().any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()) == word)
}

fn any_word(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| has_word(text, w))
}

// Negation-aware variant of any_word. A directive that says "Do NOT write
// implementation code" or "No patches" contains the literal words "write"/
// "patch" just as much as one that actually requests them -- any_word alone
// can't tell the two apart, which is exactly what misclassified a read-only
// architectural-audit directive as a mutation task (its own prohibition
// clauses repeatedly used "write"/"patch"). This checks each occurrence of
// the target word against the 3 preceding tokens for a negation marker and
// only counts it as a real trigger if no such marker is present.
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

#[allow(dead_code)]
const NOTHING_TO_FIX_BELIEF_THRESHOLD: f32 = 0.85;

#[allow(dead_code)]
fn has_source_file_evidence(local_evidence: &str) -> bool {
    local_evidence.lines().any(|line| {
        (line.starts_with("[READ_OK]") || line.starts_with("[SEARCH_CODE_OK]"))
            && line.contains(".rs")
    }) || local_evidence.contains(".rs\n")
        || local_evidence.contains(".rs:")
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

        // ⚡ FIX: Removed first-word recon classification.
        // We now prioritize Mutation/Write checks. If ANY unnegated mutation word exists,
        // it is a mutation task, regardless of whether it starts with "read" or "analyze".

        if any_word_unnegated(
            classify_text,
            &[
                "write",
                "writing",
                "wrote",
                "write_file",
                "output",
                "outputting",
                "export",
                "exporting",
                "save",
                "saving",
                "implement",
                "implementing",
                "integrate",
                "integrating",
                "add",
                "adding",
                "create",
                "creating",
                "generate",
                "generating",
                "produce",
                "producing",
                "append",
                "appending",
                "build",
                "building",
            ],
        ) {
            conditions.push(CompletionCondition::WriteMarkerAcquired);
            is_mutation_task = true;

            if any_word(classify_text, &["roadmap", "audit", "comparison"]) {
                conditions.push(CompletionCondition::SourceCodeEvidenceRequired);
            }
        } else if any_word_unnegated(
            classify_text,
            &[
                "patch",
                "patching",
                "patched",
                "apply",
                "applying",
                "applied",
                "fix",
                "fixing",
                "fixed",
                "remediate",
                "remediating",
                "modify",
                "modifying",
                "edit",
                "editing",
                "change",
                "changing",
            ],
        ) {
            conditions.push(CompletionCondition::PatchMarkerAcquired);
            is_mutation_task = true;
        } else if any_word(classify_text, &["search", "find", "locate"]) {
            conditions.push(CompletionCondition::SearchMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else if any_word(classify_text, &["read", "inspect", "audit"]) {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else {
            conditions.push(CompletionCondition::CommandMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.85));
        }

        SubgoalContract {
            node_id: node_id.to_string(),
            objective: objective.to_string(),
            conditions,
            is_mutation_task,
        }
    }

    pub fn evaluate(contract: &SubgoalContract, local_evidence: &str, belief_score: f32) -> bool {
        // ⚡ FIX: Actually enforce the contract conditions inferred earlier.
        for condition in &contract.conditions {
            match condition {
                CompletionCondition::MinBeliefScore(min_score) => {
                    if belief_score < *min_score {
                        return false;
                    }
                }
                CompletionCondition::WriteMarkerAcquired => {
                    if !local_evidence.contains("[WRITE_OK]") {
                        return false;
                    }
                }
                CompletionCondition::PatchMarkerAcquired => {
                    if !local_evidence.contains("[PATCH_FUNCTION_OK]")
                        && !local_evidence.contains("[DIFF_PATCH_OK]")
                        && !local_evidence.contains("[REPLACE_LINES_OK]")
                    {
                        return false;
                    }
                }
                CompletionCondition::ReadMarkerAcquired => {
                    if !local_evidence.contains("[READ_OK]")
                        && !local_evidence.contains("[READ_CHUNK_OK]")
                    {
                        return false;
                    }
                }
                CompletionCondition::SearchMarkerAcquired => {
                    if !local_evidence.contains("[SEARCH_CODE_OK]")
                        && !local_evidence.contains("[SEARCH_OK]")
                    {
                        return false;
                    }
                }
                _ => {} // SourceCodeEvidenceRequired / CommandMarkerAcquired handled loosely for now
            }
        }

        // Only after the contract is satisfied do we pass it to the final verifier
        let verifier_result =
            crate::outcome_verifier::OutcomeVerifier::verify(&contract.node_id, local_evidence);
        matches!(verifier_result, crate::outcome_verifier::CompletionStatus::Approved)
    }
}
