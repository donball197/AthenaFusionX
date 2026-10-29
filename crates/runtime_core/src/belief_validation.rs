use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefValidationEvent {
    pub lesson: String,
    pub error_score: f32,
}
