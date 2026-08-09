use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompletionCondition {
    ReadMarkerAcquired,
    SearchMarkerAcquired,
    WriteMarkerAcquired,
    PatchMarkerAcquired,
    CommandMarkerAcquired,
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
        Self {
            max_turns: 20,
            max_model_calls: 40,
            model_calls_made: 0,
        }
    }

    pub fn record_call(&mut self) {
        self.model_calls_made += 1;
    }

    pub fn is_exhausted(&self, current_turn: u32) -> bool {
        current_turn >= self.max_turns || self.model_calls_made >= self.max_model_calls
    }
}

pub struct SubgoalCompletionEngine;

impl SubgoalCompletionEngine {
    pub fn infer_contract(node_id: &str, objective: &str) -> SubgoalContract {
        let obj_lower = objective.trim().to_lowercase();
        let mut conditions = Vec::new();
        let mut is_mutation_task = false;

        // ⚡ SAFETY FIX: Prevent Reconnaissance Trap
        // If the objective starts with a recon verb, do not apply a mutation lock,
        // even if the word 'fix' or 'patch' appears later in the sentence.
        let is_recon_intent = obj_lower.starts_with("read") 
            || obj_lower.starts_with("analyze") 
            || obj_lower.starts_with("inspect") 
            || obj_lower.starts_with("search") 
            || obj_lower.starts_with("examine") 
            || obj_lower.starts_with("locate") || obj_lower.starts_with("profile") || obj_lower.starts_with("benchmark");

    if obj_lower.contains("graph") || obj_lower.contains("mapping") || obj_lower.contains("dependency") {
        conditions.push(CompletionCondition::WriteMarkerAcquired);
    } else if is_recon_intent {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else if obj_lower.contains("write") || obj_lower.contains("export") || obj_lower.contains("save") || obj_lower.contains("output") {
            conditions.push(CompletionCondition::WriteMarkerAcquired);
            is_mutation_task = true;
        } else if obj_lower.contains("patch") || obj_lower.contains("apply") || obj_lower.contains("fix") || obj_lower.contains("remediate") {
            conditions.push(CompletionCondition::PatchMarkerAcquired);
            is_mutation_task = true;
        } else if obj_lower.contains("search") || obj_lower.contains("find") || obj_lower.contains("locate") {
            conditions.push(CompletionCondition::SearchMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else if obj_lower.contains("read") || obj_lower.contains("inspect") || obj_lower.contains("audit") {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else {
            conditions.push(CompletionCondition::CommandMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        }

        SubgoalContract {
            node_id: node_id.to_string(),
            objective: objective.to_string(),
            conditions,
            is_mutation_task,
        }
    }

    pub fn evaluate(contract: &SubgoalContract, local_evidence: &str, belief_score: f32) -> bool {
        for cond in &contract.conditions {
            match cond {
                CompletionCondition::WriteMarkerAcquired => {
                    if !local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]")) {
                        return false;
                    }
                }
                CompletionCondition::PatchMarkerAcquired => {
                    let has_patch = local_evidence.lines().any(|l| l.starts_with("[DIFF_PATCH_OK]") || l.starts_with("[REPLACE_LINES_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[PATCH_FUNCTION_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]"));
                    if !has_patch {
                        return false;
                    }
                }
                CompletionCondition::SearchMarkerAcquired => {
                    let has_search = local_evidence.lines().any(|l| l.starts_with("[SEARCH_CODE_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[SEARCH_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[READ_CHUNK_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[READ_OK]"));
                    if !has_search {
                        return false;
                    }
                }
                CompletionCondition::ReadMarkerAcquired => {
                    let has_read = local_evidence.lines().any(|l| l.starts_with("[READ_CHUNK_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[READ_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[SEARCH_CODE_OK]"));
                    if !has_read {
                        return false;
                    }
                }
                CompletionCondition::CommandMarkerAcquired => {
                    let has_cmd = local_evidence.lines().any(|l| l.starts_with("[CMD_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[READ_CHUNK_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[SEARCH_CODE_OK]"))
                        || local_evidence.lines().any(|l| l.starts_with("[READ_OK]"));
                    if !has_cmd {
                        return false;
                    }
                }
                CompletionCondition::MinBeliefScore(min) => {
                    if belief_score < *min {
                        return false;
                    }
                }
            }
        }
        true
    }
}
