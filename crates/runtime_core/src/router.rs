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
        Self {
            max_file_chars: 1_500,
            max_obs_lines: 2,
            max_schema_chars: 400,
            max_output_tokens: 1024,
        }
    }
    pub fn standard() -> Self {
        Self {
            max_file_chars: 8_000,
            max_obs_lines: 6,
            max_schema_chars: 1_200,
            max_output_tokens: 8192,
        }
    }
    pub fn doctor() -> Self {
        Self {
            max_file_chars: 15_000,
            max_obs_lines: 10,
            max_schema_chars: 1_500,
            max_output_tokens: 8192,
        }
    }
    pub fn security() -> Self {
        Self {
            max_file_chars: 20_000,
            max_obs_lines: 10,
            max_schema_chars: 2_000,
            max_output_tokens: 8192,
        }
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
    fn resolve_max_turns() -> u8 {
        std::env::var("NEXUS_MAX_TURNS")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(8)
    }

    pub fn classify(d: &str) -> DirectiveClass {
        let d = d.to_lowercase();
        let trimmed = d.trim();

        // 1. Security Audits first (F-003): must beat TargetedEdit so
        // "security audit fix" / "cve patch" stay SecurityAudit.
        if [
            "vulnerability",
            "cve",
            "taint",
            "exploit",
            "sqli",
            "security audit",
            "penetration test",
        ]
        .iter()
        .any(|k| trimmed.contains(k))
            || (trimmed.contains("audit")
                && (trimmed.contains("security")
                    || trimmed.contains("system")
                    || trimmed.contains("code")))
        {
            return DirectiveClass::SecurityAudit;
        }

        // 2. Explicit Targeted Edits & Patches
        if ["fix", "patch", "edit", "replace"].iter().any(|k| trimmed.contains(k)) {
            return DirectiveClass::TargetedEdit { file_hint: "target".into() };
        }

        // 3. Feature Builds & Implementations
        if ["build", "implement", "create", "add feature", "extend", "generate"]
            .iter()
            .any(|k| trimmed.contains(k))
        {
            return DirectiveClass::FeatureBuild;
        }

        // 4. Shell / Command Operations
        if [
            "cargo check",
            "cargo test",
            "cargo build",
            "ls ",
            "find ",
            "git ",
            "cat ",
            "rm ",
            "sh ",
        ]
        .iter()
        .any(|k| trimmed.starts_with(k) || trimmed.contains(&format!(" {k}")))
        {
            return DirectiveClass::ShellOp;
        }

        // 5. Repo Doctor / Diagnostics
        if ["diagnose", "doctor", "fix repo", "suture", "repair", "preflight"]
            .iter()
            .any(|k| trimmed.contains(k))
        {
            return DirectiveClass::RepoDoctor;
        }

        // 6. Wasm Forge
        if ["wasm", "webassembly", "wat", "zig", "wasm_tester"].iter().any(|k| trimmed.contains(k))
        {
            return DirectiveClass::WasmForge;
        }

        // 7. Inquiry (Strict check: starts with question/inquiry verbs or ends with '?')
        if ["what ", "why ", "how ", "explain ", "describe ", "status ", "show ", "who "]
            .iter()
            .any(|k| trimmed.starts_with(k))
            || trimmed.ends_with('?')
        {
            return DirectiveClass::Inquiry;
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

        // F-002: same authority as AgentFactory (pin → .env → cascade → global).
        let (primary_model, budget) = match class {
            DirectiveClass::SecurityAudit => {
                (nexus_model_hub::factory::get_heavy_model(), ContextBudget::security())
            }
            DirectiveClass::RepoDoctor => {
                (nexus_model_hub::factory::get_heavy_model(), ContextBudget::doctor())
            }
            DirectiveClass::Inquiry => {
                (nexus_model_hub::factory::get_flash_lite_model(), ContextBudget::minimal())
            }
            _ => (nexus_model_hub::factory::get_flash_lite_model(), ContextBudget::standard()),
        };

        AgentProfile {
            class,
            primary_model,
            max_turns: Self::resolve_max_turns(),
            budget,
            persona: persona.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_classification_precedence() {
        assert_eq!(
            IntelligentRouter::classify("Fix memory leak in parser"),
            DirectiveClass::TargetedEdit { file_hint: "target".into() }
        );
        assert_eq!(
            IntelligentRouter::classify("Perform full system security audit"),
            DirectiveClass::SecurityAudit
        );
        // F-003: security terms beat "fix"/"patch"
        assert_eq!(
            IntelligentRouter::classify("security audit fix for CVE-2024-0001"),
            DirectiveClass::SecurityAudit
        );
        // "perform " contains the characters "rm ", but is not a shell command.
        assert_ne!(IntelligentRouter::classify("Perform a system review"), DirectiveClass::ShellOp);
        assert_eq!(
            IntelligentRouter::classify("Explain how the router works"),
            DirectiveClass::Inquiry
        );
        assert_eq!(IntelligentRouter::classify("cargo test --workspace"), DirectiveClass::ShellOp);
    }
}
