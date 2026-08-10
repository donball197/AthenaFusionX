use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CognitiveEvent {
    GoalCreated { goal_id: Uuid, objective: String, timestamp: u64 },
    GoalCompleted { goal_id: Uuid, timestamp: u64 },
    TaskScheduled { task_id: String, objective: String, required_agent: String },
    HypothesisCreated { session_id: Uuid, hypothesis_id: Uuid, hypothesis: String },
    BeliefUpdated { session_id: Uuid, hypothesis_id: Uuid, new_belief: f32 },
    ActionProposed { session_id: Uuid, proposed_action: String },
    ActionApproved { command_id: Uuid, capability: String },
    ActionDenied { command_id: Uuid, reason: String },
    ObservationRecorded { session_id: Uuid, evidence: String, modifier: f32 },
    MemoryCommitted { key: String, importance: f32 },
}
