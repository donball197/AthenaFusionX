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
        if combined.contains("timeout") || 
           combined.contains("connection reset") || 
           combined.contains("rate limit") || 
           combined.contains("503 service unavailable") ||
           combined.contains("429 too many requests") {
            return VerificationResult::Retry("Transient network/API fault detected.".into());
        }

        // 2. Deterministic Faults -> Immediate Escalation
        if combined.contains("error:") || 
           combined.contains("cannot find crate") || 
           combined.contains("borrowed value") || 
           combined.contains("syntax error") ||
           combined.contains("test failed") ||
           combined.contains("panicked at") {
            return VerificationResult::Escalate("Deterministic compilation/logic fault detected.".into());
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

        // TODO: Phase 2 Low-temp Gemma API semantic verification.
        VerificationResult::Success 
    }
}
