// src/agent_protocol.rs — v4.8 [FULLY RECONCILED]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[derive(PartialEq)]
pub enum AgentAction {
    #[serde(rename = "propose_evolution")] ProposeEvolution { target: String, diff: String, risk: f32 },
    #[serde(rename = "synthesize_policy")] SynthesizePolicy { id: String, logic: String, objective: String },
    #[serde(rename = "execute_command")] ExecuteCommand { command: String },
    #[serde(rename = "read_file")] ReadFile { path: String },
    #[serde(rename = "write_file")] WriteFile { path: String, content: String },
    #[serde(rename = "search_memory")] SearchMemory { query: String },
    #[serde(rename = "vmo_create")] VmoCreate { size: usize },
    #[serde(rename = "vmo_write")] VmoWrite { handle: String, data: String },
    #[serde(rename = "calculate_entropy")] CalculateEntropy { handle: String },
    #[serde(rename = "lazarus_snapshot")] LazarusSnapshot { path: String, tag: Option<String> },
    #[serde(rename = "export_report")] ExportReport { title: String, content: String },
    #[serde(rename = "analyze_image")] AnalyzeImage { path: String, query: String },
    #[serde(rename = "propose_ui_module")] ProposeUiModule { id: String, label: String, kind: String, source: String, directive: String },
    #[serde(rename = "set_stress_test")] SetStressTest { enabled: bool },
    #[serde(rename = "synthesize_wasm")] SynthesizeWasm { code: String },
    #[serde(rename = "wasm_execute_local")] WasmExecuteLocal { func_name: String, input: f32 },
    #[serde(rename = "wasm_load")] WasmLoad { name: String, func_name: String, input: f32 },
    #[serde(rename = "halt")] Halt,
}
