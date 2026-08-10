use crate::agent_protocol::AgentAction;
use crate::intent_contract::IntentContract;
use crate::policy::Capability;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MotorAction {
    ExecuteCommand { command: String },
    WriteFile { path: String, content: String },
    ReadFile { path: String },
    ReadFileChunk { path: String, start_line: usize, max_lines: usize },
    SearchCode { path: String, query: String },
    SearchMemory { query: String },
    ReplaceLines { path: String, start_line: usize, end_line: usize, replacement: String },
    RunTests { test_target: String },
    DiffPatch { path: String, search: String, replace: String },
    PatchFunction { path: String, function_name: String, new_body: String },
    Halt { reasoning: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorCommand {
    pub command_id: Uuid,
    pub intent_id: Uuid,
    pub goal_id: Option<Uuid>,
    pub action: MotorAction,
    pub capability: Capability,
    pub expected_outcome: String,
    pub created_at: u64,
}

pub struct MotorCortex;

impl MotorCortex {
    pub fn translate(
        intent: &IntentContract,
        goal_id: Option<Uuid>,
        action: AgentAction,
        capability: Capability,
        expected_outcome: String,
    ) -> MotorCommand {
        let motor_action = match action {
            AgentAction::ExecuteCommand { command, .. } => MotorAction::ExecuteCommand { command },
            AgentAction::WriteFile { path, content, .. } => {
                MotorAction::WriteFile { path, content }
            }
            AgentAction::ReadFile { path, .. } => MotorAction::ReadFile { path },
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => {
                MotorAction::ReadFileChunk { path, start_line, max_lines }
            }
            AgentAction::SearchCode { path, query, .. } => MotorAction::SearchCode { path, query },
            AgentAction::SearchMemory { query, .. } => MotorAction::SearchMemory { query },
            AgentAction::ReplaceLines { path, start_line, end_line, replacement, .. } => {
                MotorAction::ReplaceLines { path, start_line, end_line, replacement }
            }
            AgentAction::RunTests { test_target, .. } => MotorAction::RunTests { test_target },
            AgentAction::DiffPatch { path, search, replace, .. } => {
                MotorAction::DiffPatch { path, search, replace }
            }
            AgentAction::PatchFunction { path, function_name, new_body, .. } => {
                MotorAction::PatchFunction { path, function_name, new_body }
            }
            AgentAction::Halt { reasoning } => MotorAction::Halt { reasoning },
            _ => MotorAction::Halt { reasoning: "Fallback halt for unmapped action".to_string() },
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        MotorCommand {
            command_id: Uuid::new_v4(),
            intent_id: intent.id,
            goal_id,
            action: motor_action,
            capability,
            expected_outcome,
            created_at: now,
        }
    }
}
