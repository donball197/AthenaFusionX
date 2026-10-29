use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefTransitionEvent {
    pub belief_id: Uuid,
    pub lesson: String,
    pub new_confidence: f32,
}
