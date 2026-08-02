use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyDecision {
    Continue,
    Modify,
    Escalate,
    Abandon,
}

pub struct StrategyRevision;

impl StrategyRevision {
    pub fn decide(confidence: f32) -> StrategyDecision {
        if confidence > 0.75 {
            StrategyDecision::Continue
        } else if confidence > 0.45 {
            StrategyDecision::Modify
        } else if confidence > 0.20 {
            StrategyDecision::Escalate
        } else {
            StrategyDecision::Abandon
        }
    }
}
