use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalAdaptationAction {
    ContinueGoal,

    ModifyStrategy,

    PauseGoal,

    TerminateGoal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAdaptationDecision {
    pub id: Uuid,

    pub goal_id: Uuid,

    pub action: GoalAdaptationAction,

    pub reason: String,

    pub timestamp: i64,
}

pub struct GoalAdaptationEngine;

impl GoalAdaptationEngine {
    pub fn from_belief_action(
        goal_id: Uuid,
        action: crate::belief_policy::BeliefAction,
    ) -> GoalAdaptationDecision {
        let (adaptation, reason) = match action {
            crate::belief_policy::BeliefAction::Continue => {
                (GoalAdaptationAction::ContinueGoal, "Belief confidence supports continuation")
            }

            crate::belief_policy::BeliefAction::Adapt => (
                GoalAdaptationAction::ModifyStrategy,
                "Belief degradation requires strategy adaptation",
            ),

            crate::belief_policy::BeliefAction::Suspend => (
                GoalAdaptationAction::PauseGoal,
                "Belief confidence insufficient, pausing objective",
            ),

            crate::belief_policy::BeliefAction::Abandon => {
                (GoalAdaptationAction::TerminateGoal, "Evidence no longer supports objective")
            }
        };

        GoalAdaptationDecision {
            id: Uuid::new_v4(),

            goal_id,

            action: adaptation,

            reason: reason.to_string(),

            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
