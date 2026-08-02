use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryScope {
    pub target_directories: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanningConstraints {
    pub max_execution_time_ms: u64,
    pub allowed_mutations: bool,
    pub strict_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutputFormat {
    pub format_type: String,
    pub verbosity_level: u8,
    pub include_metadata: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntelligencePlanner {
    pub id: String,
    pub scope: RepositoryScope,
    pub constraints: PlanningConstraints,
    pub format: OutputFormat,
}
