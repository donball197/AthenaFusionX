// src/agent_protocol.rs — v5.0 SOVEREIGN ACTION SCHEMA
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AgentStatus {
    Idle,
    Executing,
    Fault(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AgentTaskType {
    Refactor,
    Audit,
    SystemCheck,
    NeuralInference,
    Command,
    DevOps,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentDirective {
    pub id: Uuid,
    pub task_type: AgentTaskType,
    pub payload: String,
    pub priority: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum AgentAction {
    #[serde(rename = "scrape_url")]
    ScrapeUrl { url: String },

    #[serde(rename = "execute_command")]
    ExecuteCommand { command: String },

    #[serde(rename = "vmo_create")]
    VmoCreate,

    #[serde(rename = "vmo_clone")]
    VmoClone { parent_handle: String },

    #[serde(rename = "vmo_write")]
    VmoWrite { handle: String, data: String },

    #[serde(rename = "vmo_read")]
    VmoRead { handle: String },

    #[serde(rename = "get_vmo_state")]
    GetVmoState,

    #[serde(rename = "write_file")]
    WriteFile { path: String, content: String },

    #[serde(rename = "search_memory")]
    SearchMemory { query: String },

    #[serde(rename = "lazarus_snapshot")]
    LazarusSnapshot { path: String, tag: Option<String> },

    #[serde(rename = "git_action")]
    GitAction { command: String, message: Option<String> },

    #[serde(rename = "export_report")]
    ExportReport { title: String, content: String },

    // ⚡ V14: WASM Synthesis Action
    #[serde(rename = "synthesize_wasm")]
    SynthesizeWasm { code: String },

    #[serde(rename = "halt")]
    Halt,
}
