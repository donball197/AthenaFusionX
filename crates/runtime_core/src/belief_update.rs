use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdate {
    pub id: Uuid,

    pub belief_id: String,

    pub evidence: String,

    pub confidence_delta: f32,

    pub source_memory: Option<Uuid>,

    pub timestamp: i64,
}

impl BeliefUpdate {
    pub fn new(belief_id: &str, evidence: &str, delta: f32) -> Self {
        Self {
            id: Uuid::new_v4(),
            belief_id: belief_id.to_string(),
            evidence: evidence.to_string(),
            confidence_delta: delta,
            source_memory: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
