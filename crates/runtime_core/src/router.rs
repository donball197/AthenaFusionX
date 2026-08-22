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
            max_output_tokens: 4096,
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
    pub fn classify(d: &str) -> DirectiveClass {
        let d = d.to_lowercase();
        if ["diagnose", "doctor", "fix repo", "suture"].iter().any(|k| d.contains(k)) {
            return DirectiveClass::RepoDoctor;
        }
        if ["cargo check", "ls ", "find "].iter().any(|k| d.contains(k)) {
            return DirectiveClass::ShellOp;
        }
        if ["fix", "patch", "edit"].iter().any(|k| d.contains(k)) {
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
]

CRITICAL RULES:
1. NEVER use `write_file` to modify an existing function. You MUST use `patch_function` or `diff_patch`.
2. `patch_function` requires `path`, `function_name`, and the full replacement code in `new_body`.
3. `diff_patch` requires `search` (the exact old string) and `replace` (the exact new string). DO NOT output unified diffs or use a `diff` field.
4. When you have achieved the task objective, include the `halt` action in your JSON array.
5. You MUST escape all newlines inside JSON strings as `\n` (e.g. "line1\nline2").
6. CRITICAL HALT OVERRIDE: You are strictly forbidden from issuing a [HALT] command if your objective requires code modification (patching). Finding the target code via 'grep' or 'cat' is only reconnaissance. You MUST verify the presence of a successful [PATCH_FUNCTION_OK], [WRITE_OK], or [DIFF_PATCH_OK] in your physical evidence buffer before you are authorized to halt. Halting before patching is considered a critical mission failure.
7. STRICT SCHEMA COMPLIANCE: NEVER mix tool arguments. `write_file` takes `path` and `content`. `run_tests` takes `test_target`. DO NOT hallucinate or combine fields across tools.
8. ATHENA OMNISCIENCE: You are fully aware of your compiled workspace sub-agents (impact_analyzer, etc.). Use them by issuing an `execute_command` with their binary path.
9. ANTI-LOOP DIRECTIVE: If you have already read or searched a file and discovered a vulnerability, DO NOT search or read it again. You MUST immediately use `diff_patch` or `patch_function` to fix the vulnerability."#;

        let (primary_model, budget) = match class {
            DirectiveClass::RepoDoctor | DirectiveClass::SecurityAudit => {
                let model = std::env::var("NEXUS_HEAVY_MODEL")
                    .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".to_string());
                (model, ContextBudget::doctor())
            }
            _ => {
                let model = std::env::var("NEXUS_LIGHT_MODEL")
                    .unwrap_or_else(|_| "gemini-3.5-flash-lite".to_string());
                (model, ContextBudget::standard())
            }
        };

        AgentProfile { class, primary_model, max_turns: 8, budget, persona: persona.to_string() }
    }
}
