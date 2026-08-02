use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentContract {
    pub id: Uuid,

    /// Human/cognitive objective
    pub objective: String,

    /// Current confidence that this intent is correct
    pub confidence: f32,

    /// Safety classification
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,

    Medium,

    High,
}

impl IntentContract {
    pub fn new(objective: &str, confidence: f32) -> Self {
        Self {
            id: Uuid::new_v4(),

            objective: objective.to_string(),

            confidence,

            risk_level: RiskLevel::Medium,
        }
    }
}
