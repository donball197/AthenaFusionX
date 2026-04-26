// src/router.rs — Intelligent Agent Router v4.1 [REPO_DOCTOR_ACTIVE]
// Logic: Task-specific budget allocation with strict JSON enforcement.

#[derive(Debug, Clone, PartialEq)]
pub enum DirectiveClass {
    ShellOp,
    TargetedEdit { file_hint: String },
    RepoDoctor, // ⚡ NEW: Automated Workspace Healing
    SecurityAudit,
    WasmForge, 
    Inquiry,
    FeatureBuild,
    General,
}

#[derive(Debug, Clone)]
pub struct ContextBudget {
    pub max_file_chars: usize,
    pub max_obs_lines: usize,
    pub max_schema_chars: usize,
    pub max_output_tokens: u32,
}

impl ContextBudget {
    pub fn minimal() -> Self {
        Self { max_file_chars: 1_500, max_obs_lines: 2, max_schema_chars: 400, max_output_tokens: 1024 }
    }
    pub fn standard() -> Self {
        Self { max_file_chars: 8_000, max_obs_lines: 6, max_schema_chars: 1_200, max_output_tokens: 4096 }
    }
    pub fn doctor() -> Self {
        Self { max_file_chars: 15_000, max_obs_lines: 10, max_schema_chars: 1_500, max_output_tokens: 8192 }
    }
    pub fn security() -> Self {
        Self { max_file_chars: 20_000, max_obs_lines: 10, max_schema_chars: 2_000, max_output_tokens: 8192 }
    }
}

pub struct AgentProfile {
    pub class: DirectiveClass,
    pub primary_model: &'static str,
    pub max_turns: u8,
    pub budget: ContextBudget,
    pub persona: String,
}

pub struct IntelligentRouter;

impl IntelligentRouter {
    pub fn classify(actual_directive: &str) -> DirectiveClass {
        let d = actual_directive.to_lowercase();

        if ["diagnose", "doctor", "fix repo", "repair workspace", "suture"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::RepoDoctor;
        }
        if ["synthesize", "forge", "wasm cell"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::WasmForge;
        }
        if ["cargo check", "cargo test", "git status", "ls ", "find "].iter().any(|k| d.contains(k)) {
            return DirectiveClass::ShellOp;
        }
        if ["audit ", "vulnerability", "memory safe"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::SecurityAudit;
        }
        if ["fix", "patch", "update", "edit"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::TargetedEdit { file_hint: "target".into() };
        }
        DirectiveClass::General
    }

    pub fn route(directive: &str) -> AgentProfile {
        let start_idx = directive.find("DIRECTIVE: ").map(|i| i + 11).unwrap_or(0);
        let user_cmd = &directive[start_idx..];

        let class = Self::classify(user_cmd);

        match class {
            DirectiveClass::RepoDoctor => AgentProfile {
                class: DirectiveClass::RepoDoctor,
                primary_model: "gemma-4-31b-it", // ⚡ High-reasoning model
                max_turns: 8,
                budget: ContextBudget::doctor(),
                persona: "IDENTITY: NEXUS Repo Doctor. Goal: Diagnose and patch compilation errors. RULE: You MUST output a valid JSON array inside an <ACTION> block. WORKFLOW: 1. read_file on the failing file. 2. write_file with the corrected code. 3. execute_command with 'cargo check' to verify. 4. halt only when integrity passes.".into(),
            },
            DirectiveClass::WasmForge => AgentProfile {
                class: DirectiveClass::WasmForge,
                primary_model: "gemma-4-26b-a4b-it",
                max_turns: 5,
                budget: ContextBudget::standard(),
                persona: "IDENTITY: NEXUS Forge Master. Goal: Synthesize Rust WASM cells. RULE: You MUST output a valid JSON array inside an <ACTION> block containing {\"type\": \"synthesize_wasm\", \"code\": \"...\"}. DO NOT write markdown tutorials. DO NOT output WAT format. RULE 2: You MUST execute {\"type\": \"halt\"} immediately after a successful synthesis.".into(),
            },
            DirectiveClass::ShellOp => AgentProfile {
                class: DirectiveClass::ShellOp,
                primary_model: "gemma-4-26b-a4b-it",
                max_turns: 6,
                budget: ContextBudget::minimal(),
                persona: "IDENTITY: NEXUS DevOps. Goal: Substrate maintenance. RULE: You MUST execute operations using the <ACTION> JSON array.".into(),
            },
            _ => AgentProfile {
                class: DirectiveClass::General,
                primary_model: "gemma-4-26b-a4b-it",
                max_turns: 8,
                budget: ContextBudget::standard(),
                persona: "IDENTITY: NEXUS Sovereign Agent. Goal: Fulfill the directive. RULE: You MUST execute operations using the <ACTION> JSON array. NEVER output conversational text outside of <THOUGHT>.".into(),
            },
        }
    }

    pub fn build_prompt(directive: &str, profile: &AgentProfile, obs: &[String]) -> String {
        let history = obs.iter().map(|o| format!(">> {}", o)).collect::<Vec<_>>().join("\n");
        format!(
            "{}\n\nMISSION HISTORY:\n{}\n\nCURRENT OBJECTIVE: {}\n\nNEXT STEP (Output <THOUGHT> then <ACTION> JSON array):",
            profile.persona, history, directive
        )
    }
}
