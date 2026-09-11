use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerificationResult {
    Success,
    Retry(String),
    Failure(String),
    Escalate(String),
}

pub struct Verifier;

impl Verifier {
    /// LocalDiagnosis Layer: Cheap, deterministic heuristic matching.
    pub fn diagnose_output(stderr: &str, stdout: &str) -> VerificationResult {
        let combined = format!("{} {}", stderr, stdout).to_lowercase();

        // 1. Transient Faults -> Yields a localized Retry
        if combined.contains("timeout")
            || combined.contains("connection reset")
            || combined.contains("rate limit")
            || combined.contains("503 service unavailable")
            || combined.contains("429 too many requests")
        {
            return VerificationResult::Retry("Transient network/API fault detected.".into());
        }

        // 2. Deterministic Faults -> Immediate Escalation
        if combined.contains("error:")
            || combined.contains("cannot find crate")
            || combined.contains("borrowed value")
            || combined.contains("syntax error")
            || combined.contains("test failed")
            || combined.contains("panicked at")
        {
            return VerificationResult::Escalate(
                "Deterministic compilation/logic fault detected.".into(),
            );
        }

        // 3. Ambiguous Anomaly -> Escalate
        if !stderr.trim().is_empty() {
            return VerificationResult::Escalate("Unknown stderr anomaly detected.".into());
        }

        VerificationResult::Success
    }

    pub async fn evaluate(stdout: &str, stderr: &str, criteria: &[String]) -> VerificationResult {
        let diagnosis = Self::diagnose_output(stderr, stdout);
        if diagnosis != VerificationResult::Success {
            return diagnosis;
        }
        if criteria.is_empty() {
            return VerificationResult::Escalate("Node has no explicit success criteria.".into());
        }

        Self::evaluate_contract_criteria(stdout, criteria)
    }

    /// Deterministic contract verification based on explicit node criteria.
    ///
    /// Explicit success criteria are authoritative. Legacy node-id rules are
    /// retained only for older persisted DAG nodes that have no criteria.
    pub fn verify_contract(
        node_id: &str,
        evidence: &str,
        criteria: &[String],
    ) -> VerificationResult {
        if !criteria.is_empty() {
            return Self::evaluate_contract_criteria(evidence, criteria);
        }
        // Define hard-coded rules per node type.
        // For simplicity, we use node_id prefixes; in production, use a proper rule registry.
        if node_id.starts_with("clone_") || node_id.starts_with("clone_openfang") {
            // Verify that /tmp/openfang exists
            if evidence.contains("[CMD_OK]") && evidence.contains("/tmp/openfang") {
                return VerificationResult::Success;
            } else {
                return VerificationResult::Retry(
                    "Clone failed or /tmp/openfang not found".to_string(),
                );
            }
        } else if node_id.starts_with("inspect_") || node_id.contains("workspace_topology") {
            // Verify that Cargo.toml was read
            if evidence.contains("[READ_OK]") && evidence.contains("/tmp/openfang/Cargo.toml") {
                return VerificationResult::Success;
            } else {
                return VerificationResult::Retry("Cargo.toml not read".to_string());
            }
        } else if node_id.starts_with("analyze_") || node_id.contains("wasm_watchdog") {
            // Verify that SECURITY.md was read
            if evidence.contains("[READ_OK]") && evidence.contains("/tmp/openfang/SECURITY.md") {
                return VerificationResult::Success;
            } else {
                return VerificationResult::Retry("SECURITY.md not read".to_string());
            }
        } else if node_id.starts_with("generate_") || node_id.contains("roadmap") {
            // Verify that ROADMAP_OPENFANG_COMPARISON.md exists in the root
            // We'll check evidence for a write_file action that created it.
            if evidence.contains("[WRITE_OK]")
                && evidence.contains("ROADMAP_OPENFANG_COMPARISON.md")
            {
                // Additional check: does the file contain required sections? (optional)
                return VerificationResult::Success;
            } else {
                return VerificationResult::Retry("Roadmap file not written".to_string());
            }
        }
        // Unknown nodes fail closed. A verifier must never silently
        // authorize an unrecognized node.
        VerificationResult::Escalate(format!(
            "No verification contract exists for node '{}'",
            node_id
        ))
    }

    fn evaluate_contract_criteria(evidence: &str, criteria: &[String]) -> VerificationResult {
        if criteria.is_empty() {
            return VerificationResult::Escalate(
                "Cannot verify node: success criteria are empty.".into(),
            );
        }

        for criterion in criteria {
            let criterion = criterion.trim();

            let satisfied = if criterion == "NODE_OBJECTIVE_EXECUTED_AND_VERIFIED" {
                evidence.contains("[DAG NODE COMPLETE]")
            } else if let Some(path) = criterion.strip_prefix("READ_OK:") {
                evidence.contains("[READ_OK]") && evidence.contains(path)
            } else if let Some(target) = criterion.strip_prefix("READ_FUNCTION_OK:") {
                evidence.contains("[READ_FUNCTION_OK]") && evidence.contains(target)
            } else if let Some(target) = criterion.strip_prefix("SEARCH_CODE_OK:") {
                evidence.contains("[SEARCH_CODE_OK]") && evidence.contains(target)
            } else if criterion == "SEARCH_MEMORY_OK" {
                evidence.contains("[SEARCH_MEMORY_OK]")
            } else if let Some(path) = criterion.strip_prefix("WRITE_OK:") {
                evidence.contains("[WRITE_OK]") && evidence.contains(path)
            } else if criterion == "CMD_OK" {
                evidence.contains("[CMD_OK]")
            } else if criterion == "DAG_LINEAGE_OK" {
                evidence.contains("[DAG LINEAGE INHERITED EVIDENCE]")
                    && evidence.contains("[DAG LINEAGE COMPLETE]")
            } else {
                return VerificationResult::Escalate(format!(
                    "Unknown success criterion: {}",
                    criterion
                ));
            };

            if !satisfied {
                return VerificationResult::Retry(format!(
                    "Mandatory success criterion not satisfied: {}",
                    criterion
                ));
            }
        }

        VerificationResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::{VerificationResult, Verifier};

    #[test]
    fn search_memory_ok_requires_physical_evidence() {
        let criteria = vec!["SEARCH_MEMORY_OK".to_string()];

        let success = Verifier::verify_contract(
            "task_1",
            "[SEARCH_MEMORY_OK] Found semantic context: host observations involving uptime, filesystem, available memory, and load average",
            &criteria,
        );
        assert_eq!(success, VerificationResult::Success);

        let empty = Verifier::verify_contract(
            "task_1",
            "[SEARCH_MEMORY_EMPTY] Semantic search completed successfully but returned zero matches",
            &criteria,
        );
        assert!(matches!(empty, VerificationResult::Retry(_)));

        let unrelated =
            Verifier::verify_contract("task_1", "[SEARCH_CODE_OK] host observations", &criteria);
        assert!(matches!(unrelated, VerificationResult::Retry(_)));
    }
}
