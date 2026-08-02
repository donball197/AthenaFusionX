#[derive(Debug, Clone)]
pub struct VerificationReport {
    pub evidence_quality: f32,
    pub mutation_detected: bool,
    pub command_completed: bool,
    pub security_violation_detected: bool,
    pub goal_verified: bool,
}

impl VerificationReport {
    pub fn from_evidence(evidence: &str, evaluator_confidence: f32) -> Self {
        let security_violation = evidence.contains("PATH_TRAVERSAL_DENIED")
            || evidence.contains("sandbox_violation")
            || evidence.contains("SECURITY_VIOLATION");

        let command_completed = (evidence.contains("[CMD_OK]")
            || evidence.contains("[READ_OK]")
            || evidence.contains("[WRITE_OK]"))
            && !security_violation;

        Self {
            evidence_quality: if security_violation { 0.0 } else { evaluator_confidence.max(0.0) },
            mutation_detected: evidence.contains("[WRITE_OK]")
                || evidence.contains("[PATCH_FUNCTION_OK]")
                || evidence.contains("[DIFF_PATCH_OK]"),
            command_completed,
            security_violation_detected: security_violation,
            goal_verified: evaluator_confidence > 0.5 && !security_violation,
        }
    }

    pub fn can_complete(&self, belief: f32) -> bool {
        !self.security_violation_detected
            && self.command_completed
            && belief >= 0.85
            && self.goal_verified
            && self.evidence_quality >= 0.5
    }
}
