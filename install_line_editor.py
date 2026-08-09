import os

print(">> Installing 'replace_lines' Surgical Editing Tool...")

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
    '    #[serde(rename = "replace_lines")]\n    ReplaceLines {\n        path: String,\n        start_line: usize,\n        end_line: usize,\n        replacement: String,\n        #[serde(default)]\n        reasoning: String,\n    },\n    #[serde(rename = "diff_patch")]'
)

# 2. Update Motor Cortex
patch_file(
    "crates/runtime_core/src/motor_cortex.rs",
    '    DiffPatch { path: String, search: String, replace: String },',
    '    ReplaceLines { path: String, start_line: usize, end_line: usize, replacement: String },\n    DiffPatch { path: String, search: String, replace: String },'
)
patch_file(
    "crates/runtime_core/src/motor_cortex.rs",
    '            AgentAction::DiffPatch { path, search, replace, .. } => MotorAction::DiffPatch { path, search, replace },',
    '            AgentAction::ReplaceLines { path, start_line, end_line, replacement, .. } => MotorAction::ReplaceLines { path, start_line, end_line, replacement },\n            AgentAction::DiffPatch { path, search, replace, .. } => MotorAction::DiffPatch { path, search, replace },'
)

# 3. Update Policy / Capabilities
patch_file(
    "crates/runtime_core/src/policy.rs",
    '"write_file" | "diff_patch" | "patch_function" | "lazarus_snapshot"\n            | "export_report" => Capability::WriteSubstrate,',
    '"write_file" | "diff_patch" | "patch_function" | "replace_lines" | "lazarus_snapshot"\n            | "export_report" => Capability::WriteSubstrate,'
)
patch_file(
    "crates/runtime_core/src/policy.rs",
    '    ToolContract {\n        name: "write_file",',
    '    ToolContract {\n        name: "replace_lines",\n        binary_path: "internal::agent_action",\n        flags: &["path", "start_line", "end_line", "replacement"],\n        usage_pattern: "replace_lines <path> <start_line> <end_line> <replacement> (Surgically replaces lines without touching the rest of the file)",\n    },\n    ToolContract {\n        name: "write_file",'
)

# 4. Update Action Gate
patch_file(
    "crates/runtime_core/src/action_gate.rs",
    '            MotorAction::DiffPatch { .. } => {',
    '            MotorAction::ReplaceLines { .. } => {\n                self.sandbox.validate_intent("replace_lines")\n                    .map_err(|e| anyhow::anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;\n            }\n\n            MotorAction::DiffPatch { .. } => {'
)

# 5. Update Executor (Physical Logic)
executor_old = '                MotorAction::DiffPatch { path, search, replace } => {'
executor_new = '''                MotorAction::ReplaceLines { path, start_line, end_line, replacement } => {
                    let safe_path = sanitize(&path)?;
                    let content = tokio::fs::read_to_string(&safe_path).await?;
                    let lines: Vec<&str> = content.split('\\n').collect();
                    let start_idx = start_line.saturating_sub(1);
                    let end_idx = end_line.saturating_sub(1);
                    
                    if start_idx > lines.len() || start_idx > end_idx {
                        return Err(anyhow::anyhow!("Invalid line range for replacement"));
                    }
                    let end_idx_actual = std::cmp::min(end_idx, lines.len().saturating_sub(1));
                    
                    let before = lines[..start_idx].to_vec();
                    let after = if end_idx_actual + 1 < lines.len() { lines[end_idx_actual + 1..].to_vec() } else { vec![] };
                    
                    let mut new_lines = before;
                    new_lines.push(replacement.as_str());
                    new_lines.extend(after);
                    
                    tokio::fs::write(&safe_path, new_lines.join("\\n")).await?;
                    Ok(format!("[REPLACE_LINES_OK] Replaced lines {}-{} in {}", start_line, end_line, path))
                }
                MotorAction::DiffPatch { path, search, replace } => {'''
patch_file("crates/runtime_core/src/executor.rs", executor_old, executor_new)

# 6. Update Cortical Loop Success Markers (to recognize REPLACE_LINES_OK)
patch_file("crates/runtime_core/src/cortical_loop.rs", 
           'l.starts_with("[DIFF_PATCH_OK]")', 
           'l.starts_with("[DIFF_PATCH_OK]") || l.starts_with("[REPLACE_LINES_OK]")')

# 7. Update Completion Engine Success Markers
patch_file("crates/runtime_core/src/completion_engine.rs", 
           'l.starts_with("[DIFF_PATCH_OK]")', 
           'l.starts_with("[DIFF_PATCH_OK]") || l.starts_with("[REPLACE_LINES_OK]")')

# 8. Update Epistemic Engine Prompts
patch_file(
    "crates/runtime_core/src/epistemic_engine.rs",
    '             5. [{ "type": "diff_patch",',
    '             4.5. [{ "type": "replace_lines", "path": "<filepath>", "start_line": 10, "end_line": 20, "replacement": "<new_code>", "reasoning": "..." }]\n             5. [{ "type": "diff_patch",'
)
patch_file(
    "crates/runtime_core/src/epistemic_engine.rs",
    '- write_file <path> <content>: Write content to a file.',
    '- replace_lines <path> <start_line> <end_line> <replacement>: Surgically replace lines to bypass token limits.\n- write_file <path> <content>: Write content to a file.'
)
patch_file(
    "crates/runtime_core/src/epistemic_engine.rs",
    "'diff_patch', 'patch_function'",
    "'replace_lines', 'diff_patch', 'patch_function'"
)
patch_file(
    "crates/runtime_core/src/epistemic_engine.rs",
    "[DIFF_PATCH_OK], [PATCH_FUNCTION_OK]",
    "[REPLACE_LINES_OK], [DIFF_PATCH_OK], [PATCH_FUNCTION_OK]"
)

print(">> Line Editor Installation Complete.")
