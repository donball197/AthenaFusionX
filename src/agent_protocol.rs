// src/agent_protocol.rs — v5.1 SOVEREIGN ACTION SCHEMA
// NEW: ReadFile, WasmExecuteLocal, WasmStore, WasmLoad actions.
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

    // NEW: ReadFile — reads a file and returns content as an observation
    // Does NOT write anything. Safe for inspection before write_file.
    #[serde(rename = "read_file")]
    ReadFile { path: String },

    #[serde(rename = "write_file")]
    WriteFile { path: String, content: String },

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

    #[serde(rename = "search_memory")]
    SearchMemory { query: String },

    #[serde(rename = "lazarus_snapshot")]
    LazarusSnapshot { path: String, tag: Option<String> },

    #[serde(rename = "git_action")]
    GitAction { command: String, message: Option<String> },

    #[serde(rename = "export_report")]
    ExportReport { title: String, content: String },

    // Synthesize a WASM cell from Rust code — returns GENESIS_PAYLOAD alias
    #[serde(rename = "synthesize_wasm")]
    SynthesizeWasm { code: String },

    // Execute the most recently synthesized WASM cell by alias (no raw b64)
    // Use this instead of execute_distributed_wasm after a synthesize_wasm.
    #[serde(rename = "wasm_execute_local")]
    WasmExecuteLocal { func_name: String, input: f32 },

    // Store a synthesized WASM cell under a friendly name in the registry.
    #[serde(rename = "wasm_store")]
    WasmStore { name: String, func_name: String },

    // Load a previously stored WASM cell by name and execute it.
    #[serde(rename = "wasm_load")]
    WasmLoad { name: String, func_name: String, input: f32 },

    // Execute WASM on the distributed cluster (raw b64 or alias)
    #[serde(rename = "execute_distributed_wasm")]
    ExecuteDistributedWasm { wasm_b64: String, func_name: String, input: f32 },

    #[serde(rename = "halt")]
    Halt,
}
