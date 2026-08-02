use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildIntent {
    pub objective: String,

    pub expected_capability: String,

    pub success_condition: String,
}

impl BuildIntent {
    pub fn controlled(objective: &str) -> Self {
        Self {
            objective: objective.to_string(),

            expected_capability: "verified artifact".into(),

            success_condition: "cargo check and tests pass".into(),
        }
    }
}
