// crates/runtime_core/src/cortical/terminal_gate.rs

use crate::cognition::epistemic_session::EpistemicSession;

/// Authoritative evaluator for terminal completion conditions across cognitive states.
#[allow(dead_code)]
pub struct TerminalGate {
    pub contract_satisfied: bool,
    pub compile_remediation_resolved: bool,
    pub is_command_only: bool,
    pub requires_semantic_verification: bool,
}

impl TerminalGate {
    pub fn evaluate(session: &EpistemicSession, belief: f32) -> Self {
        let contract_satisfied = crate::completion_engine::SubgoalCompletionEngine::evaluate(
            &session.contract,
            &session.local_evidence,
            belief,
        );
        let compile_remediation_resolved = !session.compile_remediation.is_active();
        let is_command_only = session.contract.conditions.len() == 1
            && matches!(
                session.contract.conditions.first(),
                Some(
                    crate::completion_engine::CompletionCondition::CommandExecutionVerified { .. }
                )
            );
        let requires_semantic_verification =
            crate::completion_engine::requires_semantic_verification(&session.contract.objective);

        // P1-b: Artifact content recheck. When the contract requires a
        // write artifact, read the file and verify it contains key claims
        // from the objective. This closes the false-positive gap where
        // CommandMarkerAcquired + WritePathAcquired is satisfied but the
        // file content doesn't match the task description (e.g. pivot_test.rs
        // "returns 2" was never verified).
        let content_recheck_ok = if contract_satisfied {
            verify_artifact_content(&session.contract, &session.local_evidence)
        } else {
            true // not satisfied yet — recheck not needed
        };

        let contract_satisfied = contract_satisfied && content_recheck_ok;

        Self {
            contract_satisfied,
            compile_remediation_resolved,
            is_command_only,
            requires_semantic_verification,
        }
    }

    pub fn is_autonomous_completion_satisfied(&self) -> bool {
        self.contract_satisfied
            && self.compile_remediation_resolved
            && !self.requires_semantic_verification
    }

    #[allow(dead_code)] // sibling of is_autonomous_completion_satisfied without the remediation gate; intentionally unwired
    pub fn is_pure_command_satisfied(&self) -> bool {
        self.contract_satisfied && self.is_command_only && !self.requires_semantic_verification
    }
}

/// P1-b: Read artifact files and verify they contain key claims from
/// the objective. Returns false if the file exists but is missing
/// expected content.
fn verify_artifact_content(
    contract: &crate::completion_engine::SubgoalContract,
    _evidence: &str,
) -> bool {
    use crate::completion_engine::CompletionCondition;

    for condition in &contract.conditions {
        if let CompletionCondition::WritePathAcquired { path } = condition {
            // Read the file back from disk
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue, // can't read — don't block on IO errors
            };

            // Extract key numeric/identifier claims from the objective
            // and verify they appear in the file.
            // E.g. "returns 2" -> check file contains "2"
            // E.g. "fn a() -> i32 { 1 }" -> check file contains "fn a"
            let obj_lower = contract.objective.to_lowercase();

            // Check for "returns N" patterns
            for word in contract.objective.split_whitespace() {
                if word.len() == 1 && word.chars().all(|c| c.is_ascii_digit()) {
                    // Single digit — likely a return value claim
                    // Only check if "return" appears near it
                    if obj_lower.contains("return") || obj_lower.contains("->") {
                        if !content.contains(word) {
                            return false;
                        }
                    }
                }
            }

            // Check for "fn name()" patterns
            for word in contract.objective.split_whitespace() {
                if word.starts_with("fn ") || (word.starts_with("fn") && word.len() > 2) {
                    let fname = word.trim_start_matches("fn").trim_start_matches(' ');
                    let fname = fname.split('(').next().unwrap_or("");
                    if !fname.is_empty() && !content.contains(fname) {
                        return false;
                    }
                }
            }

            // Check for "def name()" patterns (Python)
            for word in contract.objective.split_whitespace() {
                if word == "def" {
                    // Find the function name in the objective
                    if let Some(pos) = contract.objective.find("def ") {
                        let rest = &contract.objective[pos + 4..];
                        let fname = rest.split('(').next().unwrap_or("").trim();
                        if !fname.is_empty() && !content.contains(fname) {
                            return false;
                        }
                    }
                }
            }
        }
    }

    true // no content mismatches found
}
