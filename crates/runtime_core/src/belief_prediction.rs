use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefPrediction {
    pub belief_id: Uuid,

    pub predicted_outcome: String,

    pub confidence: f32,
}

impl BeliefPrediction {
    pub fn new(belief_id: Uuid, predicted_outcome: String, confidence: f32) -> Self {
        Self { belief_id, predicted_outcome, confidence }
    }
}
