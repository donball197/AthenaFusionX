import os

# 1. Update task_manager to separate the Law from the Mission
fpath = "src/task_manager.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()
    
    # SUTURE: Use raw directive in the loop, move law to system_prompt
    code = code.replace(
        'let directive = cli::inject_workspace_dna(raw_directive).await;',
        'let mission_directive = raw_directive.to_string();\n    let execution_law = cli::get_execution_law_v6().await;'
    )
    code = code.replace(
        'system_prompt: Some("You are NEXUS, a precise autonomous coding agent. Follow the execution law strictly.".to_string()),',
        'system_prompt: Some(execution_law.clone()),'
    )
    code = code.replace(
        'let result = orchestrator.run(directive.clone(), ctx, client, api_key, Some(tx.clone())).await;',
        'let result = orchestrator.run(mission_directive.clone(), ctx, client, api_key, Some(tx.clone())).await;'
    )
    
    with open(fpath, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] Context routing cleaned in task_manager.rs\x1b[0m")

# 2. Add the law helper to cli.rs
fpath = "src/cli.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()
    
    if "pub async fn get_execution_law_v6()" not in code:
        law_fn = """
pub async fn get_execution_law_v6() -> String {
    r#"[NEXUS EXECUTION LAW v6.5 — PURE JSON MODE]
IDENTITY: Precise autonomous coding agent.
MISSION: Complete the USER DIRECTIVE using filesystem and shell actions.

CRITICAL RULES:
- Output ONLY a raw JSON array of AgentAction objects.
- NO markdown, NO code fences, NO preamble, NO conversational text.
- Valid actions: execute_command, write_file, read_file, halt.
- Every 'write_file' MUST be followed by 'cargo check'.
- Output '[]' if no action is required.
- Detailed reasoning is REQUIRED inside each JSON object.

Output ONLY the JSON array."#.to_string()
}
"""
        with open(fpath, "a") as f:
            f.write(law_fn)
        print("\x1b[32m>> [SUTURE] Execution Law v6.5 established in cli.rs\x1b[0m")
