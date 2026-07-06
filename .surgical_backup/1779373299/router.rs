// src/router.rs — Intelligent Agent Router v4.1
// use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum DirectiveClass {
    ShellOp,
    TargetedEdit { file_hint: String },
    RepoDoctor,
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
    pub fn minimal() -> Self { Self { max_file_chars: 1_500, max_obs_lines: 2, max_schema_chars: 400, max_output_tokens: 1024 } }
    pub fn standard() -> Self { Self { max_file_chars: 8_000, max_obs_lines: 6, max_schema_chars: 1_200, max_output_tokens: 4096 } }
    pub fn doctor() -> Self { Self { max_file_chars: 15_000, max_obs_lines: 10, max_schema_chars: 1_500, max_output_tokens: 8192 } }
    pub fn security() -> Self { Self { max_file_chars: 20_000, max_obs_lines: 10, max_schema_chars: 2_000, max_output_tokens: 8192 } }
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
                class,
                primary_model: "gemma-4-26b-a4b-it",
                max_turns: 8,
                budget: ContextBudget::doctor(),
                persona: "IDENTITY: NEXUS Repo Doctor. Goal: Diagnose and patch compilation errors.".into(),
            },
            _ => AgentProfile {
                class,
                primary_model: "gemma-4-26b-a4b-it",
                max_turns: 8,
                budget: ContextBudget::standard(),
                persona: "IDENTITY: NEXUS Sovereign Agent. Goal: Fulfill the directive.".into(),
            },
        }
    }
}
