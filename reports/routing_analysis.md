[
  {
    "type": "patch_function",
    "path": "/root/AthenaFusionx/crates/runtime_core/src/router.rs",
    "function_name": "classify",
    "new_body": "pub fn classify(d: &str) -> DirectiveClass {\n    let d = d.to_lowercase();\n    if [\"audit\", \"vulnerability\", \"cve\", \"taint\", \"exploit\", \"sqli\", \"security\"].iter().any(|k| d.contains(k)) {\n        return DirectiveClass::SecurityAudit;\n    }\n    if [\"wasm\", \"webassembly\", \"wat\", \"zig\", \"wasm_tester\"].iter().any(|k| d.contains(k)) {\n        return DirectiveClass::WasmForge;\n    }\n    if [\"diagnose\", \"doctor\", \"fix repo\", \"suture\", \"repair\"].iter().any(|k| d.contains(k)) {\n        return DirectiveClass::RepoDoctor;\n    }\n    if [\"what\", \"why\", \"how\", \"explain\", \"describe\", \"status\", \"show\", \"who\"].iter().any(|k| d.starts_with(k) || d.contains(k)) {\n        return DirectiveClass::Inquiry;\n    }\n    if [\"cargo check\", \"ls \", \"find \", \"git \", \"cat \"].iter().any(|k| d.starts_with(k) || d.contains(k)) {\n        return DirectiveClass::ShellOp;\n    }\n    if [\"build\", \"implement\", \"create\", \"add feature\", \"extend\", \"generate\"].iter().any(|k| d.contains(k)) {\n        return DirectiveClass::FeatureBuild;\n    }\n    if [\"fix\", \"patch\", \"edit\", \"replace\"].iter().any(|k| d.contains(k)) {\n        return DirectiveClass::TargetedEdit { file_hint: \"target\".into() };\n    }\n    DirectiveClass::General\n}",
    "reasoning": "The provided 'classify' function correctly identifies the intent of the directive by checking for specific keywords in the input string. It maps these keywords to the appropriate 'DirectiveClass' enum values, which is the expected behavior for a classification function in this context."
  },
  {
    "type": "patch_function",
    "path": "/root/AthenaFusionx/crates/runtime_core/src/router.rs",
    "function_name": "route",
    "new_body": "pub fn route(directive: &str) -> AgentProfile {\n    let class = Self::classify(directive);\n    let persona = r#\"IDENTITY: NEXUS Sovereign Agent.
STRICT SYSTEM RULE: You MUST output EXACTLY ONE valid JSON array of AgentAction objects. No markdown.

MANDATORY SCHEMA:
[
  { \"type\": \"patch_function\", \"path\": \"...\", \"function_name\": \"...\", \"new_body\": \"...\", \"reasoning\": \"...\" },
  { \"type\": \"diff_patch\", \"path\": \"...\", \"search\": \"...\", \"replace\": \"...\", \"reasoning\": \"...\" },
  { \"type\": \"write_file\", \"path\": \"...\", \"content\": \"...\", \"reasoning\": \"...\" },
  { \"type\": \"run_tests\", \"test_target\": \"...\", \"reasoning\": \"...\" },
  { \"type\": \"read_file\", \"path\": \"...\", \"reasoning\": \"...\" },
  { \"type\": \"execute_command\", \"command\": \"...\", \"reasoning\": \"...\" },
  { \"type\": \"search_memory\", \"query\": \"...\", \"reasoning\": \"...\" },
  { \"type\": \"halt\", \"reasoning\": \"...\" }
]\"#;\n\n    let (primary_model, budget) = match class {\n        DirectiveClass::SecurityAudit => (std::env::var(\"NEXUS_HEAVY_MODEL\").unwrap_or_default(), ContextBudget::security()),\n        DirectiveClass::RepoDoctor => (std::env::var(\"NEXUS_HEAVY_MODEL\").unwrap_or_default(), ContextBudget::doctor()),\n        DirectiveClass::Inquiry => (std::env::var(\"NEXUS_LIGHT_MODEL\").unwrap_or_default(), ContextBudget::minimal()),\n        _ => (std::env::var(\"NEXUS_LIGHT_MODEL\").unwrap_or_default(), ContextBudget::standard()),\n    };\n\n    AgentProfile { class, primary_model, max_turns: 8, budget, persona: persona.to_string() }\n}",
    "reasoning": "The 'route' function successfully creates an 'AgentProfile' by first classifying the directive using the 'classify' method. It then selects the appropriate primary model and context budget based on the determined class. The function correctly populates the 'AgentProfile' struct with the necessary parameters, including the persona string."
  }
]