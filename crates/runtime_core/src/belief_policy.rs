use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BeliefAction {
    Continue,
    Adapt,
    Suspend,
    Abandon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefDecision {
    pub id: Uuid,

    pub belief_id: Uuid,

    pub confidence: f32,

    pub action: BeliefAction,

    pub reason: String,

    pub timestamp: i64,
}

pub struct BeliefDecisionPolicy;

impl BeliefDecisionPolicy {
    pub fn evaluate(belief_id: Uuid, confidence: f32) -> BeliefDecision {
        let (action, reason) = if confidence >= 0.75 {
            (BeliefAction::Continue, "Confidence sufficient to continue objective".to_string())
        } else if confidence >= 0.45 {
            (
                BeliefAction::Adapt,
                "Confidence degraded, strategy adjustment recommended".to_string(),
            )
        } else if confidence >= 0.20 {
            (BeliefAction::Suspend, "Confidence too low, suspend objective".to_string())
        } else {
            (BeliefAction::Abandon, "Evidence no longer supports objective".to_string())
        };

        Self::decision(belief_id, confidence, action, reason)
    }

    fn decision(
        belief_id: Uuid,
        confidence: f32,
        action: BeliefAction,
        reason: String,
    ) -> BeliefDecision {
        BeliefDecision {
            id: Uuid::new_v4(),

            belief_id,

            confidence,

            action,

            reason,

            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
