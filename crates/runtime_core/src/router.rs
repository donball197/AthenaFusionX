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
    pub primary_model: String,
    pub max_turns: u8,
    pub budget: ContextBudget,
    pub persona: String,
}

pub struct IntelligentRouter;
impl IntelligentRouter {
    pub fn classify(d: &str) -> DirectiveClass {
        let d = d.to_lowercase();
        if ["audit", "vulnerability", "cve", "taint", "exploit", "sqli", "security"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::SecurityAudit;
        }
        if ["wasm", "webassembly", "wat", "zig", "wasm_tester"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::WasmForge;
        }
        if ["diagnose", "doctor", "fix repo", "suture", "repair"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::RepoDoctor;
        }
        if ["what", "why", "how", "explain", "describe", "status", "show", "who"].iter().any(|k| d.starts_with(k) || d.contains(k)) {
            return DirectiveClass::Inquiry;
        }
        if ["cargo check", "ls ", "find ", "git ", "cat "].iter().any(|k| d.starts_with(k) || d.contains(k)) {
            return DirectiveClass::ShellOp;
        }
        if ["build", "implement", "create", "add feature", "extend", "generate"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::FeatureBuild;
        }
        if ["fix", "patch", "edit", "replace"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::TargetedEdit { file_hint: "target".into() };
        }
        DirectiveClass::General
    }

    pub fn route(directive: &str) -> AgentProfile {
        let class = Self::classify(directive);
        let persona = r#"IDENTITY: NEXUS Sovereign Agent.
STRICT SYSTEM RULE: You MUST output EXACTLY ONE valid JSON array of AgentAction objects. No markdown.

MANDATORY SCHEMA:
[
  { "type": "patch_function", "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." },
  { "type": "diff_patch", "path": "...", "search": "...", "replace": "...", "reasoning": "..." },
  { "type": "write_file", "path": "...", "content": "...", "reasoning": "..." },
  { "type": "run_tests", "test_target": "...", "reasoning": "..." },
  { "type": "read_file", "path": "...", "reasoning": "..." },
  { "type": "execute_command", "command": "...", "reasoning": "..." },
  { "type": "search_memory", "query": "...", "reasoning": "..." },
  { "type": "halt", "reasoning": "..." }
]"#;

        let (primary_model, budget) = match class {
            DirectiveClass::SecurityAudit => (
                std::env::var("NEXUS_HEAVY_MODEL").unwrap_or_default(),
                ContextBudget::security()
            ),
            DirectiveClass::RepoDoctor => (
                std::env::var("NEXUS_HEAVY_MODEL").unwrap_or_default(),
                ContextBudget::doctor()
            ),
            DirectiveClass::Inquiry => (
                std::env::var("NEXUS_LIGHT_MODEL").unwrap_or_default(),
                ContextBudget::minimal()
            ),
            _ => (
                std::env::var("NEXUS_LIGHT_MODEL").unwrap_or_default(),
                ContextBudget::standard()
            ),
        };

        AgentProfile { class, primary_model, max_turns: 8, budget, persona: persona.to_string() }
    }
}
