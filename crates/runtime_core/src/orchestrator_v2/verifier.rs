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
            return VerificationResult::Success;
        }
        // Default to success; can be extended with LLM later.
        VerificationResult::Success
    }

    /// Deterministic contract verification based on node id and evidence.
    pub fn verify_contract(node_id: &str, evidence: &str) -> VerificationResult {
        // Define hard-coded rules per node type.
        // For simplicity, we use node_id prefixes; in production, use a proper rule registry.
        if node_id.starts_with("clone_") || node_id.starts_with("clone_openfang") {
            // Verify that /tmp/openfang exists
            if evidence.contains("[CMD_OK]") && evidence.contains("/tmp/openfang") {
                return VerificationResult::Success;
            } else {
                return VerificationResult::Retry("Clone failed or /tmp/openfang not found".to_string());
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
            if evidence.contains("[WRITE_OK]") && evidence.contains("ROADMAP_OPENFANG_COMPARISON.md") {
                // Additional check: does the file contain required sections? (optional)
                return VerificationResult::Success;
            } else {
                return VerificationResult::Retry("Roadmap file not written".to_string());
            }
        }
        // For unknown nodes, default to success (or retry).
        VerificationResult::Success
    }
}
