use crate::agent_protocol::AgentAction;
use crate::intent_contract::IntentContract;
use crate::policy::Capability;
use serde::{Deserialize, Serialize};

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
    AtomicPatch { patches: Vec<crate::agent_protocol::FilePatch> },
    DiffPatch { path: String, search: String, replace: String },
    PatchFunction { path: String, function_name: String, new_body: String },
    PatchFile { path: String, target: String, replacement: String },
    SynthesizeReport { output_path: String, source_files: Vec<String> },
    PolyglotAstPatch { path: String, pattern_name: String, patch_snippet: String },
    Halt { reasoning: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorCommand {
    pub intent_id: String,
    pub parent_action_id: Option<String>,
    pub action: MotorAction,
    pub required_capability: Capability,
    pub rationale: String,
}

pub struct MotorCortex;

impl MotorCortex {
    pub fn translate(
        intent: &IntentContract,
        parent_action_id: Option<String>,
        action: AgentAction,
        capability: Capability,
        rationale: String,
    ) -> MotorCommand {
        let motor_action = match action {
            AgentAction::ExecuteCommand { command, .. } => MotorAction::ExecuteCommand { command },
            AgentAction::WriteFile { path, content, .. } => MotorAction::WriteFile { path, content },
            AgentAction::ReadFile { path, .. } => MotorAction::ReadFile { path },
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => MotorAction::ReadFileChunk { path, start_line, max_lines },
            AgentAction::SearchCode { path, query, .. } => MotorAction::SearchCode { path, query },
            AgentAction::SearchMemory { query, .. } => MotorAction::SearchMemory { query },
            AgentAction::ReplaceLines { path, start_line, end_line, replacement, .. } => MotorAction::ReplaceLines { path, start_line, end_line, replacement },
            AgentAction::RunTests { test_target, .. } => MotorAction::RunTests { test_target },
            AgentAction::AtomicPatch { patches, .. } => MotorAction::AtomicPatch { patches },
            AgentAction::DiffPatch { path, search, replace, .. } => MotorAction::DiffPatch { path, search, replace },
            AgentAction::PatchFunction { path, function_name, new_body, .. } => MotorAction::PatchFunction { path, function_name, new_body },
            AgentAction::SynthesizeReport { output_path, source_files, .. } => MotorAction::SynthesizeReport { output_path, source_files },
            AgentAction::Halt { reasoning, .. } => MotorAction::Halt { reasoning },
            _ => MotorAction::Halt { reasoning: "Unmapped reflex action".to_string() },
        };

        MotorCommand {
            intent_id: intent.id.to_string(),
            parent_action_id,
            action: motor_action,
            required_capability: capability,
            rationale,
        }
    }
}
