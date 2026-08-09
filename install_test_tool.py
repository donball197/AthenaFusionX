import os

print(">> Installing 'run_tests' Toolsmith Upgrade...")

def patch_file(path, old, new):
    if not os.path.exists(path):
        print(f"[ERROR] {path} not found.")
        return
    with open(path, "r") as f:
        content = f.read()
    if old in content:
        content = content.replace(old, new)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Patched {path}")
    else:
        print(f"[!] Target signature not found in {path}. Skipping.")

# 1. Update Agent Protocol
patch_file(
    "crates/runtime_core/src/agent_protocol.rs",
    '    #[serde(rename = "diff_patch")]',
    '    #[serde(rename = "run_tests")]\n    RunTests {\n        test_target: String,\n        #[serde(default)]\n        reasoning: String,\n    },\n    #[serde(rename = "diff_patch")]'
)

# 2. Update Motor Cortex
patch_file(
    "crates/runtime_core/src/motor_cortex.rs",
    '    DiffPatch { path: String, search: String, replace: String },',
    '    RunTests { test_target: String },\n    DiffPatch { path: String, search: String, replace: String },'
)
patch_file(
    "crates/runtime_core/src/motor_cortex.rs",
    '            AgentAction::DiffPatch { path, search, replace, .. } => MotorAction::DiffPatch { path, search, replace },',
    '            AgentAction::RunTests { test_target, .. } => MotorAction::RunTests { test_target },\n            AgentAction::DiffPatch { path, search, replace, .. } => MotorAction::DiffPatch { path, search, replace },'
)

# 3. Update Policy / Capabilities
patch_file(
    "crates/runtime_core/src/policy.rs",
    '"execute_command" | "git_rollback" => Capability::ExecuteSystemCall,',
    '"execute_command" | "run_tests" | "git_rollback" => Capability::ExecuteSystemCall,'
)
patch_file(
    "crates/runtime_core/src/policy.rs",
    '    ToolContract {\n        name: "write_file",',
    '    ToolContract {\n        name: "run_tests",\n        binary_path: "internal::agent_action",\n        flags: &["test_target"],\n        usage_pattern: "run_tests <test_target> (Runs cargo test on the specified package/module to verify code logic. Leave blank to test the whole workspace)",\n    },\n    ToolContract {\n        name: "write_file",'
)

# 4. Update Action Gate
patch_file(
    "crates/runtime_core/src/action_gate.rs",
    '            MotorAction::DiffPatch { .. } => {',
    '            MotorAction::RunTests { .. } => {\n                self.sandbox.validate_intent("run_tests")\n                    .map_err(|e| anyhow::anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;\n            }\n\n            MotorAction::DiffPatch { .. } => {'
)

# 5. Update Executor
executor_old = '                MotorAction::DiffPatch { path, search, replace } => {'
executor_new = '''                MotorAction::RunTests { test_target } => {
                    let cmd = if test_target.is_empty() || test_target == "." {
                        "cargo test --workspace --color never 2>&1".to_string()
                    } else {
                        format!("cargo test {} --color never 2>&1", test_target)
                    };
                    
                    let out = crate::podman::PodmanShield::execute(&cmd).unwrap_or_else(|e| {
                        std::process::Output {
                            status: std::os::unix::process::ExitStatusExt::from_raw(1),
                            stdout: Vec::new(),
                            stderr: format!("TEST HARNESS FAILED: {}", e).into_bytes(),
                        }
                    });
                    
                    let output_str = String::from_utf8_lossy(&out.stdout);
                    let err_str = String::from_utf8_lossy(&out.stderr);
                    let combined = format!("STDOUT:\\n{}\\nSTDERR:\\n{}", output_str, err_str);
                    
                    let trunc = if combined.len() > 6000 {
                        format!("{}...\\n[OUTPUT TRUNCATED]", &combined[combined.len()-6000..])
                    } else {
                        combined
                    };
                    
                    if out.status.success() {
                        Ok(format!("[TEST_OK] Tests passed for target '{}':\\n{}", test_target, trunc))
                    } else {
                        Ok(format!("[TEST_FAILED] Tests failed for target '{}':\\n{}", test_target, trunc))
                    }
                }
                MotorAction::DiffPatch { path, search, replace } => {'''
patch_file("crates/runtime_core/src/executor.rs", executor_old, executor_new)

# 6. Update Interceptor
patch_file(
    "crates/runtime_core/src/action_intercept.rs",
    '    // 3. Diff Patcher',
    '    if let Some(stripped) = clean_action.strip_prefix("run_tests ") {\n        return Some(AgentAction::RunTests {\n            test_target: stripped.trim().trim_matches(|c| c == \'\\\'\' || c == \'"\').to_string(),\n            reasoning: "Intercepted fallback run_tests".to_string(),\n        });\n    }\n    if clean_action == "run_tests" {\n        return Some(AgentAction::RunTests {\n            test_target: "".to_string(),\n            reasoning: "Intercepted fallback run_tests".to_string(),\n        });\n    }\n\n    // 3. Diff Patcher'
)

# 7. Update JSON Schema
patch_file(
    "crates/runtime_core/src/epistemic_engine.rs",
    '             10. [{ "type": "halt", "reasoning": "..." }]\n',
    '             10. [{ "type": "run_tests", "test_target": "<package_or_module>", "reasoning": "..." }]\n             11. [{ "type": "halt", "reasoning": "..." }]\n'
)

print(">> Toolsmith Installation Complete.")
