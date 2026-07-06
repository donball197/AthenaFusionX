// src/router.rs — Intelligent Agent Router v3.1
// Logic: Task-specific budget allocation.

#[derive(Debug, Clone, PartialEq)]
pub enum DirectiveClass {
    ShellOp,
    TargetedEdit { file_hint: String },
    ArchitecturalChange,
    SecurityAudit,
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
    pub fn security() -> Self { Self { max_file_chars: 20_000, max_obs_lines: 10, max_schema_chars: 2_000, max_output_tokens: 8192 } }
    pub fn estimated_input_tokens(&self) -> u32 { ((self.max_file_chars + self.max_schema_chars + (self.max_obs_lines * 120)) / 4) as u32 }
}

pub struct AgentProfile {
    pub class:          DirectiveClass,
    pub primary_model:  &'static str,
    pub max_turns:      u8,
    pub budget:         ContextBudget,
    pub persona:        String,
}

pub struct IntelligentRouter;

impl IntelligentRouter {
    pub fn classify(actual_directive: &str) -> DirectiveClass {
        let d = actual_directive.to_lowercase();

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
        // ⚡ FIX: Isolate the original command from the enriched Git schema
        let start_idx = directive.find("DIRECTIVE: ").map(|i| i + 11).unwrap_or(0);
        let user_cmd = &directive[start_idx..];

        let class = Self::classify(user_cmd);

        match class {
            DirectiveClass::SecurityAudit => AgentProfile {
                class: DirectiveClass::SecurityAudit,
                primary_model: "gemma-4-31b-it",
                max_turns: 10,
                budget: ContextBudget::security(),
                persona: "IDENTITY: NEXUS SecOps. Goal: Zero-panic substrate. Use <ACTION>.".into(),
            },
            DirectiveClass::ShellOp => AgentProfile {
                class: DirectiveClass::ShellOp,
                primary_model: "gemma-4-31b-it",
                max_turns: 6,
                budget: ContextBudget::minimal(),
                persona: "IDENTITY: NEXUS DevOps. Goal: Substrate maintenance. Use <ACTION>.".into(),
            },
            _ => AgentProfile {
                class: DirectiveClass::General,
                primary_model: "gemma-4-31b-it",
                max_turns: 8,
                budget: ContextBudget::standard(),
                persona: "IDENTITY: NEXUS Generalist. Solve the task efficiently.".into(),
            }
        }
    }

    pub fn build_prompt(directive: &str, profile: &AgentProfile, obs: &[String]) -> String {
        let history = obs.iter().map(|o| format!(">> {}", o)).collect::<Vec<_>>().join("\n");
        format!("{}\n\nMISSION HISTORY:\n{}\n\nCURRENT OBJECTIVE: {}\n\nNEXT STEP:", profile.persona, history, directive)
    }
}
