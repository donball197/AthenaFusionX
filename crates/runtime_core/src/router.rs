use crate::schema_law::SYSTEM_JSON_SCHEMA;

#[derive(Debug, Clone, PartialEq)]
pub enum DirectiveClass { ShellOp, TargetedEdit { file_hint: String }, RepoDoctor, SecurityAudit, WasmForge, Inquiry, FeatureBuild, General }

#[derive(Debug, Clone)]
pub struct ContextBudget { pub max_file_chars: usize, pub max_obs_lines: usize, pub max_schema_chars: usize, pub max_output_tokens: u32 }

impl ContextBudget {
    pub fn minimal() -> Self { Self { max_file_chars: 1_500, max_obs_lines: 2, max_schema_chars: 400, max_output_tokens: 1024 } }
    pub fn standard() -> Self { Self { max_file_chars: 8_000, max_obs_lines: 6, max_schema_chars: 1_200, max_output_tokens: 4096 } }
    pub fn doctor() -> Self { Self { max_file_chars: 15_000, max_obs_lines: 10, max_schema_chars: 1_500, max_output_tokens: 8192 } }
    pub fn security() -> Self { Self { max_file_chars: 20_000, max_obs_lines: 10, max_schema_chars: 2_000, max_output_tokens: 8192 } }
}

pub struct AgentProfile { pub class: DirectiveClass, pub primary_model: &'static str, pub max_turns: u8, pub budget: ContextBudget, pub persona: String }

pub struct IntelligentRouter;
impl IntelligentRouter {
    pub fn classify(d: &str) -> DirectiveClass {
        let d = d.to_lowercase();
        if ["diagnose", "doctor", "fix repo", "suture"].iter().any(|k| d.contains(k)) { return DirectiveClass::RepoDoctor; }
        if ["cargo check", "ls ", "find "].iter().any(|k| d.contains(k)) { return DirectiveClass::ShellOp; }
        if ["fix", "patch", "edit"].iter().any(|k| d.contains(k)) { return DirectiveClass::TargetedEdit { file_hint: "target".into() }; }
        DirectiveClass::General
    }

    pub fn route(directive: &str) -> AgentProfile {
        let class = Self::classify(directive);
        let persona = format!("IDENTITY: NEXUS Sovereign Agent.\nSTRICT SYSTEM RULE: You MUST output EXACTLY ONE valid JSON array of AgentAction objects. No markdown.\n\n{}\n\n6. CRITICAL HALT OVERRIDE: You are strictly forbidden from issuing a [HALT] command if your objective requires code modification (patching). Finding the target code via 'grep' or 'cat' is only reconnaissance. You MUST verify the presence of a successful [PATCH_FUNCTION_OK], [WRITE_OK], or [DIFF_PATCH_OK] in your physical evidence buffer before you are authorized to halt. Halting before patching is considered a critical mission failure.", SYSTEM_JSON_SCHEMA);

        let (primary_model, budget) = match class {
            DirectiveClass::RepoDoctor | DirectiveClass::SecurityAudit => ("gemma-4-31b-it", ContextBudget::doctor()),
            _ => ("gemini-3.1-flash-lite", ContextBudget::standard()),
        };

        AgentProfile { class, primary_model, max_turns: 8, budget, persona }
    }
}
