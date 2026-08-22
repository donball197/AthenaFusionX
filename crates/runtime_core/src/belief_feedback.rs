use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdateEvent {
    pub belief_id: Uuid,
    pub evidence: String,
    pub confidence_delta: f32,
}
