use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilePatch {
    pub path: String,
    pub search: String,
    pub replace: String,
}

fn default_confidence() -> f32 {
    0.8
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentAction {
    #[serde(rename = "execute_command")]
    ExecuteCommand {
        command: String,
        #[serde(default)]
        reasoning: String,
        #[serde(default = "default_confidence")]
        confidence: f32,
    },
    #[serde(rename = "patch_file")]
    PatchFile {
        path: String,
        target: String,
        replacement: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "read_function")]
    ReadFunction {
        path: String,
        function_name: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "read_file")]
    ReadFile {
        path: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "read_file_chunk")]
    ReadFileChunk {
        path: String,
        start_line: usize,
        max_lines: usize,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "search_code")]
    SearchCode {
        path: String,
        query: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "write_file")]
    WriteFile {
        path: String,
        #[serde(alias = "replacement", alias = "new_body")]
        content: String,
        #[serde(default)]
        reasoning: String,
        #[serde(default = "default_confidence")]
        confidence: f32,
    },
    #[serde(rename = "replace_lines")]
    ReplaceLines {
        path: String,
        start_line: usize,
        end_line: usize,
        #[serde(alias = "content", alias = "new_body")]
        replacement: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "run_tests")]
    RunTests {
        test_target: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "atomic_patch")]
    AtomicPatch {
        patches: Vec<FilePatch>,
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
    #[serde(rename = "patch_function")]
    PatchFunction {
        path: String,
        function_name: String,
        #[serde(alias = "replacement", alias = "content")]
        new_body: String,
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
    #[serde(rename = "scrape_url")]
    ScrapeUrl {
        url: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "git_rollback")]
    GitRollback {
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "spawn_subagent")]
    SpawnSubagent {
        instruction: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "delegate_task")]
    DelegateTask {
        instruction: String,
        #[serde(default)]
        reasoning: String,
    },

    #[serde(rename = "synthesize_report")]
    SynthesizeReport {
        output_path: String,
        source_files: Vec<String>,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = "halt")]
    Halt {
        #[serde(default)]
        reasoning: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentReturn {
    pub status: String,
    pub output: String,
}
