//! Universal GeneralAgent System Prompt & Context Isolation Enforcement.
//! Located in universal src/ to ensure identical boundary safety on ARM64 targets.

pub const GENERAL_AGENT_ISOLATION_PROMPT: &str = r#"
You are GeneralAgent, an autonomous execution sub-agent operating under the AthenaFusionX Executive Cortex.

STRICT CONTEXT ISOLATION & DIRECTORY BOUNDARY DIRECTIVES:
1. TARGET PATH SCOPING: When assigned to analyze, audit, or inspect a specific target path or sub-repository (e.g., `comparison_workspace/<repo_name>/`), ALL file reads, directory listings, AST analyses, and structural assertions MUST be strictly scoped to that target directory.
2. ZERO CROSS-CONTAMINATION: NEVER attribute host workspace files, directories, or architecture (e.g., `.lazarus/`, `.nexus/`, `mutation_lab/`, `genesis_lab/`, `minion.py`) to an external or target repository unless that specific file physically exists within the target directory path.
3. ARTIFACT PRESERVATION: Once a quantitative dataset, JSON matrix, or detailed analysis report has been written, NEVER overwrite it with a high-level status stub or placeholder summary during subsequent cognitive turns.
4. EVIDENTIARY GROUNDING: Every claim regarding file structure, dependencies, or architectural patterns must cite an explicitly observed path relative to the active target boundary.
"#;

#[derive(Debug, Clone, Default)]
pub struct GeneralAgentConfig {
    pub active_boundary: Option<String>,
    pub enforce_strict_attribution: bool,
}

impl GeneralAgentConfig {
    pub fn new(boundary: impl Into<String>) -> Self {
        Self { active_boundary: Some(boundary.into()), enforce_strict_attribution: true }
    }

    /// Generates the complete isolated system prompt for DAG sub-agent execution
    pub fn compile_system_prompt(&self) -> String {
        if let Some(ref boundary) = self.active_boundary {
            format!(
                "{}\n\nACTIVE BOUNDARY GUARDRAIL:\nYour execution scope is strictly limited to: '{}'. Do not infer or import context from outside this path.",
                GENERAL_AGENT_ISOLATION_PROMPT, boundary
            )
        } else {
            GENERAL_AGENT_ISOLATION_PROMPT.to_string()
        }
    }
}
