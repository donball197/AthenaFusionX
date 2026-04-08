use crate::agent_protocol::AgentAction;
use crate::server::AppState;
use crate::component_runner::{AgentHandle, Capabilities};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum StepStatus {
    Pending, Running, Done, Failed(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MissionStep {
    pub id: usize,
    pub description: String,
    pub action: Option<AgentAction>,
    pub status: StepStatus,
    pub result: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Mission {
    pub id: String,
    pub goal: String,
    pub steps: Vec<MissionStep>,
    pub created_at: String,
    pub completed: bool,
}

pub struct MissionPlanner;

impl MissionPlanner {
    pub async fn execute(state: &Arc<AppState>, mut mission: Mission) -> anyhow::Result<Mission> {
        let (lifecycle_tx, _) = tokio::sync::broadcast::channel(16);

        // Spawn supervised actor handle for the entire mission
        let handle: AgentHandle = crate::component_runner::AgentComponent::spawn(
            &format!("mission-{}", mission.id),
            Capabilities::READ_FS | Capabilities::WRITE_FS,
            state.agent.clone(),
            lifecycle_tx,
        );

        for step in mission.steps.iter_mut() {
            if step.status != StepStatus::Pending { continue; }
            step.status = StepStatus::Running;

            if let Some(action) = step.action.clone() {
                match handle.execute_actions(vec![action]).await {
                    Ok(results) => {
                        step.status = StepStatus::Done;
                        step.result = Some(results.join("\n"));
                    }
                    Err(e) => {
                        step.status = StepStatus::Failed(e.to_string());
                        // Log to lifecycle for supervision
                        let _ = state.tx.send(format!("Mission step {} failed: {}", step.id, e));
                        break; // Stop on first failure (fail-fast)
                    }
                }
            }
        }

        mission.completed = mission.steps.iter().all(|s| matches!(s.status, StepStatus::Done));
        Ok(mission)
    }
}
