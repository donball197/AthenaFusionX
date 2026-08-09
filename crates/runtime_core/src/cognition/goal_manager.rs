//! Goal Manager – Phase 8B v2
//! HashMap storage for O(1) lookup.

use super::event_bus::{CognitiveEvent, CognitiveEventKind, EventBus, EventMeta};
use super::traits::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct GoalManager {
    goals: Arc<RwLock<HashMap<Uuid, Goal>>>,
    bus: EventBus,
}

impl GoalManager {
    pub fn new(bus: EventBus) -> Self {
        Self {
            goals: Arc::new(RwLock::new(HashMap::new())),
            bus,
        }
    }

    pub async fn create_goal(&self, title: &str, description: &str, priority: f32) -> Goal {
        let goal = Goal {
            id: Uuid::new_v4(),
            title: title.to_string(),
            description: description.to_string(),
            priority,
            objectives: vec![],
            status: GoalStatus::Pending,
        };
        self.goals.write().insert(goal.id, goal.clone());
        goal
    }

    pub async fn activate(&self, goal_id: Uuid) -> anyhow::Result<()> {
        let mut goals = self.goals.write();
        let g = goals
            .get_mut(&goal_id)
            .ok_or_else(|| anyhow::anyhow!("goal not found: {}", goal_id))?;
        g.status = GoalStatus::Active;
        self.bus
            .publish(CognitiveEvent {
                meta: EventMeta::new("goal_manager"),
                mission_id: None,
                kind: CognitiveEventKind::GoalActivated { goal_id },
            })
            .await;
        Ok(())
    }

    pub async fn complete(&self, goal_id: Uuid) -> anyhow::Result<()> {
        let mut goals = self.goals.write();
        let g = goals
            .get_mut(&goal_id)
            .ok_or_else(|| anyhow::anyhow!("goal not found: {}", goal_id))?;
        g.status = GoalStatus::Completed;
        self.bus
            .publish(CognitiveEvent {
                meta: EventMeta::new("goal_manager"),
                mission_id: None,
                kind: CognitiveEventKind::GoalCompleted { goal_id },
            })
            .await;
        Ok(())
    }

    pub async fn list_active(&self) -> Vec<Goal> {
        self.goals
            .read()
            .values()
            .filter(|g| g.status == GoalStatus::Active)
            .cloned()
            .collect()
    }

    pub async fn get(&self, goal_id: Uuid) -> Option<Goal> {
        self.goals.read().get(&goal_id).cloned()
    }

    pub async fn list_all(&self) -> Vec<Goal> {
        self.goals.read().values().cloned().collect()
    }
}
