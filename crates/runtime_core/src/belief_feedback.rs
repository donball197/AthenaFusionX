use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdateEvent {
    pub id: Uuid,

    pub belief_id: Uuid,

    pub evidence_source: String,

    pub previous_confidence: f32,

    pub new_confidence: f32,

    pub delta: f32,

    pub timestamp: i64,
}

impl BeliefUpdateEvent {
    pub fn significant(&self) -> bool {
        self.delta.abs() >= 0.15
    }
}
