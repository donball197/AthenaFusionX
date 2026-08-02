use crate::belief_causality::BeliefCause;

pub struct BeliefCausalityAudit;

impl BeliefCausalityAudit {
    pub fn summarize(cause: &BeliefCause) -> String {
        format!(
            "Belief changed from {:.2} to {:.2}. Cause: {}. Lesson: {}",
            cause.previous_confidence, cause.new_confidence, cause.cause, cause.lesson
        )
    }
}
