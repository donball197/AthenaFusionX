use crate::belief_delta::BeliefDelta;

pub struct BeliefAudit;

impl BeliefAudit {
    pub fn summarize(delta: &BeliefDelta) -> String {
        format!(
            "Belief changed {:.2} -> {:.2}. Evidence {:.2}, Failure {:.2}, Reflection {:.2}. Reason: {}",
            delta.previous_confidence,
            delta.new_confidence,
            delta.evidence_strength,
            delta.failure_penalty,
            delta.reflection_gain,
            delta.explanation
        )
    }
}
