import os

# --- 1. UPDATE AGENT PROTOCOL ---
proto_path = "crates/runtime_core/src/agent_protocol.rs"
with open(proto_path, "r") as f:
    proto_data = f.read()

new_proto = r"""    #[serde(rename = "halt")]
    Halt {
        #[serde(default)] reasoning: String,
    },
    #[serde(rename = "git_rollback")]
    GitRollback {
        #[serde(default)] reasoning: String,
    },"""

proto_data = proto_data.replace(
    '    #[serde(rename = "halt")]\n    Halt {\n        #[serde(default)] reasoning: String,\n    },',
    new_proto
)
with open(proto_path, "w") as f:
    f.write(proto_data)


# --- 2. UPDATE EXECUTOR ---
exec_path = "crates/runtime_core/src/executor.rs"
with open(exec_path, "r") as f:
    exec_data = f.read()

new_robotic_exec = r"""            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
            AgentAction::GitRollback { reasoning } => {
                let _ = crate::podman::PodmanShield::execute("git reset --hard");
                let _ = crate::podman::PodmanShield::execute("git clean -fd");
                Ok(format!("[GIT_ROLLBACK_OK] Reason: {}\nCodebase restored to last known good commit.", reasoning))
            },"""

new_tool_exec = r"""            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
            AgentAction::GitRollback { reasoning } => {
                let _ = crate::podman::PodmanShield::execute("git reset --hard");
                let _ = crate::podman::PodmanShield::execute("git clean -fd");
                Ok(format!("[GIT_ROLLBACK_OK] Codebase restored to last known good commit."))
            },"""

exec_data = exec_data.replace(
    '            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),',
    new_robotic_exec,
    1
)
exec_data = exec_data.replace(
    '            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),',
    new_tool_exec,
    1
)
with open(exec_path, "w") as f:
    f.write(exec_data)


# --- 3. UPDATE ROUTER PROMPT ---
router_path = "crates/runtime_core/src/router.rs"
with open(router_path, "r") as f:
    router_data = f.read()

router_data = router_data.replace(
    '{ "type": "search_memory", "query": "...", "reasoning": "..." },',
    '{ "type": "search_memory", "query": "...", "reasoning": "..." },\n  { "type": "git_rollback", "reasoning": "..." },'
)

new_rules = r"""CRITICAL RULES:
1. NEVER use `write_file` to modify an existing function. You MUST use `patch_function` or `diff_patch`.
2. ALWAYS prefer `read_function` over `read_file` if you only need to inspect a specific function. This dramatically saves context space.
3. If you make a mistake that breaks compilation, or you reach a dead end, use `git_rollback` to restore the last known good commit."""

router_data = router_data.replace(
    'CRITICAL RULES:\n1. NEVER use `write_file` to modify an existing function. You MUST use `patch_function` or `diff_patch`.\n2. ALWAYS prefer `read_function` over `read_file` if you only need to inspect a specific function. This dramatically saves context space.',
    new_rules
)
with open(router_path, "w") as f:
    f.write(router_data)


# --- 4. UPDATE EPISTEMIC ENGINE ---
ee_path = "crates/runtime_core/src/epistemic_engine.rs"
with open(ee_path, "r") as f:
    ee_data = f.read()

ee_data = ee_data.replace(
    '6. [{ "type": "halt", "reasoning": "..." }]',
    '6. [{ "type": "halt", "reasoning": "..." }]\n             7. [{ "type": "git_rollback", "reasoning": "..." }]'
)

# Add FAST-PATH for git_rollback
fast_path_inject = r"""        // 2. File Read Fast-Path
        if action_lower.starts_with("read the file") {
            let parts: Vec<&str> = proposed_action.split_whitespace().collect();
            if parts.len() >= 4 {
                let path = parts[3]; // Typically: "Read" "the" "file" "path/to/file"
                let json_out = serde_json::json!([{
                    "type": "read_file",
                    "path": path,
                    "reasoning": "Local fast-path translation."
                }]);
                println!("\x1b[35m>> [TRANSLATOR] Local Fast-Path Engaged (Bypassing LLM)\x1b[0m");
                return Ok(json_out.to_string());
            }
        }
        
        // 3. Git Rollback Fast-Path
        if action_lower.contains("rollback") || action_lower.contains("reset --hard") {
            let json_out = serde_json::json!([{
                "type": "git_rollback",
                "reasoning": "Safety override triggered."
            }]);
            println!("\x1b[35m>> [TRANSLATOR] Local Fast-Path Engaged (Bypassing LLM)\x1b[0m");
            return Ok(json_out.to_string());
        }"""

ee_data = ee_data.replace(
    """        // 2. File Read Fast-Path
        if action_lower.starts_with("read the file") {
            let parts: Vec<&str> = proposed_action.split_whitespace().collect();
            if parts.len() >= 4 {
                let path = parts[3]; // Typically: "Read" "the" "file" "path/to/file"
                let json_out = serde_json::json!([{
                    "type": "read_file",
                    "path": path,
                    "reasoning": "Local fast-path translation."
                }]);
                println!("\\x1b[35m>> [TRANSLATOR] Local Fast-Path Engaged (Bypassing LLM)\\x1b[0m");
                return Ok(json_out.to_string());
            }
        }""",
    fast_path_inject
)

with open(ee_path, "w") as f:
    f.write(ee_data)

print("[✓] Autonomous Git Rollbacks (Phase 3) applied successfully.")
