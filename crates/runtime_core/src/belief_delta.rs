use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefDelta {
    pub previous_confidence: f32,

    pub evidence_strength: f32,

    pub failure_penalty: f32,

    pub reflection_gain: f32,

    pub new_confidence: f32,

    pub explanation: String,
}

impl BeliefDelta {
    pub fn calculate(
        previous: f32,
        evidence: f32,
        failure: f32,
        reflection: f32,
        explanation: String,
    ) -> Self {
        let updated = (previous + evidence - failure + reflection).clamp(0.0, 1.0);

        Self {
            previous_confidence: previous,
            evidence_strength: evidence,
            failure_penalty: failure,
            reflection_gain: reflection,
            new_confidence: updated,
            explanation,
        }
    }
}
