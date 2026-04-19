use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum AgentAction {
    #[serde(rename = "execute_command")]
    ExecuteCommand { command: String },
    #[serde(rename = "vmo_pulse")]
    VmoPulse,
    #[serde(rename = "write_file")]
    WriteFile { path: String, content: String },
    #[serde(rename = "search_memory")]
    SearchMemory { query: String },
    #[serde(rename = "export_report")]
    ExportReport { title: String, content: String },
    #[serde(rename = "lazarus_snapshot")]
    LazarusSnapshot { path: String, tag: Option<String> },
    #[serde(rename = "halt")]
    Halt,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Idle,
    Executing,
    Fault(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentTaskType {
    Audit,
    Scout,
    Heal,
    Command,
    NexusDefault,
}

#[derive(Debug, Clone)]
pub struct AgentDirective {
    pub id: Uuid,
    pub task_type: AgentTaskType,
    pub payload: String,
    pub priority: u8,
}
