use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRecord {
    pub id: Uuid,

    pub path: String,

    pub target_function: String,

    pub method: String,

    pub verified: bool,

    pub confidence_delta: f32,

    pub timestamp: i64,
}

impl MutationRecord {
    pub fn new(path: String, function: String, method: String, verified: bool) -> Self {
        Self {
            id: Uuid::new_v4(),

            path,

            target_function: function,

            method,

            verified,

            confidence_delta: if verified { 0.05 } else { -0.20 },

            timestamp: Utc::now().timestamp(),
        }
    }
}
