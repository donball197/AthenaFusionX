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
    DataAnalysis,
}

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
    #[serde(rename = "execute_command")]
    ExecuteCommand {
        command: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "read_file")]
    ReadFile {
        path: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "write_file")]
    WriteFile {
        path: String,
        content: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "search_memory")]
    SearchMemory {
        query: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "vmo_create")]
    VmoCreate {
        size: usize,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "calculate_entropy")]
    CalculateEntropy {
        handle: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "lazarus_snapshot")]
    LazarusSnapshot {
        path: String,
        tag: Option<String>,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "export_report")]
    ExportReport {
        title: String,
        content: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "fetch_ui_snapshot")]
    FetchUiSnapshot {
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "patch_function")]
    PatchFunction {
        path: String,
        function_name: String,
        new_body: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "scrape_url")]
    ScrapeUrl {
        url: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "halt")]
    Halt {
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "diff_patch")]
    DiffPatch {
        path: String,
        search: String,
        replace: String,
        #[serde(default)]
        reasoning: String,
    },
}
