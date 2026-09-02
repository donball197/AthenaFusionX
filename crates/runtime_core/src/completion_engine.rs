use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub path: String,
    pub reason: String,
}

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
    #[serde(default)]
    pub required_artifact_paths: Vec<ArtifactSpec>,
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
        let has_write = any_word_unnegated(classify_text, &["write", "export", "save", "implement", "integrate", "add", "create", "synthesize"]);

        if has_patch || has_write {
            is_mutation_task = true;
            if has_patch {
                conditions.push(CompletionCondition::PatchMarkerAcquired);
            } else {
                conditions.push(CompletionCondition::WriteMarkerAcquired);
            }
            if any_word(classify_text, &["roadmap", "audit", "comparison", "comparative", "compare", "comparing", "compares", "compared", "versus"]) {
                conditions.push(CompletionCondition::SourceCodeEvidenceRequired);
            }
        } else {
            let first_word = classify_text.split_whitespace().next().unwrap_or("");
            let is_recon_intent = [
                "read", "analyze", "inspect", "search", "examine", "locate", "profile", "benchmark", "audit"
            ].contains(&first_word);

            let has_read_intent = any_word_unnegated(classify_text, &["read", "examine", "inspect", "analyze", "investigate", "contrast", "compare", "evaluate"]);
            let has_search_intent = any_word_unnegated(classify_text, &["search", "find", "locate"]);

            if is_recon_intent || has_read_intent || has_search_intent {
                if has_read_intent && has_search_intent {
                    conditions.push(CompletionCondition::SearchMarkerAcquired);
                    conditions.push(CompletionCondition::ReadMarkerAcquired);
                    conditions.push(CompletionCondition::MinBeliefScore(0.80));
                } else if has_search_intent {
                    conditions.push(CompletionCondition::SearchMarkerAcquired);
                    conditions.push(CompletionCondition::MinBeliefScore(0.65));
                } else {
                    conditions.push(CompletionCondition::ReadMarkerAcquired);
                    conditions.push(CompletionCondition::MinBeliefScore(0.65));
                }
            } else {
                conditions.push(CompletionCondition::CommandMarkerAcquired);
                conditions.push(CompletionCondition::MinBeliefScore(0.85));
            }
        }

        let arts = extract_required_artifact_paths(objective);
        if !arts.is_empty() && !conditions.iter().any(|c| matches!(c, CompletionCondition::PatchMarkerAcquired | CompletionCondition::WriteMarkerAcquired)) {
            conditions.push(CompletionCondition::WriteMarkerAcquired);
        }

        SubgoalContract {
            node_id: node_id.to_string(),
            objective: objective.to_string(),
            conditions,
            is_mutation_task,
            required_artifact_paths: arts,
        }
    }

    pub fn evaluate(
        contract: &SubgoalContract, 
        local_evidence: &str, 
        belief_score: f32
    ) -> bool {
        let verifier_result = crate::outcome_verifier::OutcomeVerifier::verify(
            &contract.node_id, 
            local_evidence,
            belief_score,
            &contract.conditions
        );
        matches!(verifier_result, crate::outcome_verifier::CompletionStatus::Approved)
    }
}

fn extract_required_artifact_paths(objective: &str) -> Vec<ArtifactSpec> {
    let mut out: Vec<ArtifactSpec> = Vec::new();
    let exts = ["md", "json", "rs", "toml", "yaml", "yml", "txt", "py"];
    let tokens: Vec<&str> = objective.split_whitespace().collect();

    for (i, tok) in tokens.iter().enumerate() {
        let clean = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/' && c != '_' && c != '-');
        if exts.iter().any(|ext| clean.ends_with(&format!(".{}", ext))) && (clean.contains('/') || clean.contains('.')) {
            // Check context window for read vs write
            let start = i.saturating_sub(3);
            let prev = tokens[start..i].join(" ").to_lowercase();
            let is_read_src = prev.contains("read") || prev.contains("from") || prev.contains("inspect") || prev.contains("cat");
            let is_write_tgt = prev.contains("to") || prev.contains("write") || prev.contains("save") || prev.contains("export") || prev.contains("synthesize");

            if (!is_read_src || is_write_tgt) && !out.iter().any(|a| a.path == clean) {
                out.push(ArtifactSpec {
                    path: clean.to_string(),
                    reason: "output_deliverable".into(),
                });
            }
        }
    }
    out
}
