//! Cognitive Blackboard – shared context passed between subsystems.

use super::traits::MissionState;
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct CognitiveContext {
    // Core
    pub mission: Option<MissionState>,
    pub active_goal_ids: Vec<Uuid>,
    pub working_notes: HashMap<String, String>,
    pub current_focus: Option<String>,
    pub last_decision: Option<String>,

    // Extended (optional references – populated by later phases)
    pub world_state_summary: Option<String>,
    pub active_belief_scores: HashMap<Uuid, f32>,
    pub prediction_cache: HashMap<String, String>,
    pub policy_context: Option<String>,
    pub scheduler_state: Option<String>,
    pub resource_budget_remaining: Option<f32>,
}

impl CognitiveContext {
    pub fn new() -> Self {
        Self::default()
    }
}
