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
        let persona = r#"IDENTITY: NEXUS Sovereign Agent.
STRICT SYSTEM RULE: You MUST output EXACTLY ONE valid JSON array of AgentAction objects. No markdown.

MANDATORY SCHEMA:
[
  { "type": "read_function", "path": "...", "function_name": "...", "reasoning": "..." },
  { "type": "patch_function", "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." },
  { "type": "read_file", "path": "...", "reasoning": "..." },
  { "type": "write_file", "path": "...", "content": "...", "reasoning": "..." },
  { "type": "diff_patch", "path": "...", "search": "...", "replace": "...", "reasoning": "..." },
  { "type": "execute_command", "command": "...", "reasoning": "..." },
  { "type": "search_memory", "query": "...", "reasoning": "..." },
  { "type": "git_rollback", "reasoning": "..." },
  { "type": "halt", "reasoning": "..." }
]

CRITICAL RULES:
1. NEVER use `read_file` on a `.rs` file if you only need to inspect a specific function. You MUST use `read_function` to save context space.
2. NEVER use `write_file` to modify an existing function. You MUST use `patch_function` or `diff_patch`.
3. `patch_function` requires `path`, `function_name`, and the full replacement code in `new_body`.
4. If you make a mistake that breaks compilation, or you reach a dead end, use `git_rollback` to restore the last known good commit.
5. When you have achieved the task objective, include the `halt` action in your JSON array.
6. You MUST escape all newlines inside JSON strings as `\n`."#;

        let (primary_model, budget) = match class {
            DirectiveClass::RepoDoctor | DirectiveClass::SecurityAudit => ("gemma-4-31b-it", ContextBudget::doctor()),
            _ => ("gemini-3.1-flash-lite", ContextBudget::standard()),
        };

        AgentProfile { class, primary_model, max_turns: 8, budget, persona: persona.to_string() }
    }
}
