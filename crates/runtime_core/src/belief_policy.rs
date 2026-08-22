use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BeliefAction {
    Continue,
    Adapt,
    Suspend,
    Abandon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefDecision {
    pub belief_id: Uuid,
    pub confidence: f32,
    pub action: BeliefAction,
    pub reason: String,
}

pub struct BeliefDecisionPolicy;

impl BeliefDecisionPolicy {
    /// STUB: placeholder thresholds only — not the real belief-policy engine.
    pub fn evaluate(belief_id: Uuid, confidence: f32) -> BeliefDecision {
        let (action, reason) = if confidence >= 0.75 {
            (BeliefAction::Continue, "stub: confidence above continue threshold")
        } else if confidence >= 0.5 {
            (BeliefAction::Adapt, "stub: confidence in adapt band")
        } else if confidence >= 0.25 {
            (BeliefAction::Suspend, "stub: confidence in suspend band")
        } else {
            (BeliefAction::Abandon, "stub: confidence below abandon threshold")
        };

        BeliefDecision {
            belief_id,
            confidence,
            action,
            reason: reason.to_string(),
        }
    }
}
