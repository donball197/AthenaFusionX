use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefTransitionEvent {
    pub belief_id: String,

    pub previous_confidence: f32,

    pub new_confidence: f32,

    pub evidence: String,

    pub reflection: String,

    pub lesson: String,
}

impl BeliefTransitionEvent {
    pub fn from_delta(
        belief_id: String,
        delta: crate::belief_delta::BeliefDelta,
        evidence: String,
        reflection: String,
    ) -> Self {
        Self {
            belief_id,

            previous_confidence: delta.previous_confidence,

            new_confidence: delta.new_confidence,

            evidence,

            reflection: reflection.clone(),

            lesson: format!(
                "Belief updated from {:.2} to {:.2}: {}",
                delta.previous_confidence, delta.new_confidence, reflection
            ),
        }
    }
}
