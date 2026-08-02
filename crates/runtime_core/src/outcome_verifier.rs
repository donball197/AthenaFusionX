use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeAssessment {
    pub success_probability: f32,
    pub deviation_score: f32,
    pub confidence_delta: f32,
    pub lessons: Vec<String>,
}

pub struct OutcomeVerifier;

impl OutcomeVerifier {
    pub fn evaluate(expected: &str, observed: &str) -> OutcomeAssessment {
        let success = if expected == observed { 1.0 } else { 0.5 };

        OutcomeAssessment {
            success_probability: success,
            deviation_score: 1.0 - success,
            confidence_delta: success - 0.5,
            lessons: vec![format!("Expected: {} Observed: {}", expected, observed)],
        }
    }
}
