use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefCause {
    pub id: Uuid,

    pub belief_id: Uuid,

    pub previous_confidence: f32,

    pub new_confidence: f32,

    pub evidence: String,

    pub cause: String,

    pub lesson: String,
}

impl BeliefCause {
    pub fn new(
        belief_id: Uuid,
        previous_confidence: f32,
        new_confidence: f32,
        evidence: String,
        cause: String,
        lesson: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            belief_id,
            previous_confidence,
            new_confidence,
            evidence,
            cause,
            lesson,
        }
    }
}

pub struct BeliefCausalityAudit;
impl BeliefCausalityAudit {
    pub fn summarize(cause: &BeliefCause) -> String {
        format!(
            "Belief changed from {:.2} to {:.2}. Cause: {}. Lesson: {}",
            cause.previous_confidence, cause.new_confidence, cause.cause, cause.lesson
        )
    }
}
