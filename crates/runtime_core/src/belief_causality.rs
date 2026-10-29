use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefCause {
    pub id: Uuid,
    pub cause: String,
    pub lesson: String,
    pub new_confidence: f32,
}
