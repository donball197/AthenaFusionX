use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub proof_hash: [u8; 32],
    pub metadata: HashMap<String, serde_json::Value>,
    pub confidence_score: f64,
}
