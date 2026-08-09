use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentContract {
    pub id: Uuid,
    pub rationale: String,
    pub belief_score: f32,
}

impl IntentContract {
    pub fn new(rationale: &str, belief_score: f32) -> Self {
        Self {
            id: Uuid::new_v4(),
            rationale: rationale.to_string(),
            belief_score,
        }
    }
}
