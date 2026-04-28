use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AgentStatus { Idle, Executing, Fault(String) }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AgentTaskType { Refactor, Audit, SystemCheck, NeuralInference, Command, DevOps }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentDirective {
    pub id: Uuid,
    pub task_type: AgentTaskType,
    pub payload: String,
    pub priority: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum AgentAction {
    #[serde(rename = "execute_command")] ExecuteCommand { command: String, reasoning: String },
    #[serde(rename = "read_file")] ReadFile { path: String, reasoning: String },
    #[serde(rename = "write_file")] WriteFile { path: String, content: String, reasoning: String },
    #[serde(rename = "search_memory")] SearchMemory { query: String, reasoning: String },
    #[serde(rename = "vmo_create")] VmoCreate { size: usize, reasoning: String },
    #[serde(rename = "calculate_entropy")] CalculateEntropy { handle: String, reasoning: String },
    #[serde(rename = "lazarus_snapshot")] LazarusSnapshot { path: String, tag: Option<String>, reasoning: String },
    #[serde(rename = "export_report")] ExportReport { title: String, content: String, reasoning: String },
    #[serde(rename = "fetch_ui_snapshot")] FetchUiSnapshot { reasoning: String },
    #[serde(rename = "halt")] Halt { reasoning: String },
}
