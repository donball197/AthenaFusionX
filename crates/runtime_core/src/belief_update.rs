use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdate {
    pub belief_id: String,
    pub evidence: String,
    pub confidence_delta: f32,
}

impl BeliefUpdate {
    pub fn new(belief_id: &str, evidence: &str, confidence_delta: f32) -> Self {
        Self {
            belief_id: belief_id.to_string(),
            evidence: evidence.to_string(),
            confidence_delta,
        }
    }
}
