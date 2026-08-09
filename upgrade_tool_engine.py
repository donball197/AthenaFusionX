import os

print(">> Upgrading Athena to V6 Tool Execution Engine...")

path = "crates/runtime_core/src/cortical_loop.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    # Part 1: Wire the new 'replace_lines' tool into the native fast-path
    old_intercept = """                                    } else if let Some(stripped) = action_str.strip_prefix("write_file ") {"""
    new_intercept = """                                    } else if let Some(stripped) = action_str.strip_prefix("replace_lines ") {
                                        let parts: Vec<&str> = stripped.splitn(4, ' ').collect();
                                        if parts.len() == 4 {
                                            self.log("\\x1b[32m>> [CORTEX INTERCEPT] Native Tool Intercept: Handling replace_lines directly.\\x1b[0m".to_string());
                                            actions.push(AgentAction::ReplaceLines {
                                                path: parts[0].to_string(),
                                                start_line: parts[1].parse().unwrap_or(0),
                                                end_line: parts[2].parse().unwrap_or(0),
                                                replacement: parts[3].trim().trim_start_matches(|c| c == '"' || c == '\'').trim_end_matches(|c| c == '"' || c == '\'').to_string(),
                                                reasoning: "Intercepted fallback replace_lines".to_string(),
                                            });
                                        }
                                    } else if let Some(stripped) = action_str.strip_prefix("write_file ") {"""

    # Part 2: Build the Bash Leakage Guard
    old_fallback = """                                    } else {
                                        self.log("\\x1b[33m>> [CORTEX FALLBACK] Translator produced empty JSON. Executing proposed command directly...\\x1b[0m".to_string());
                                        actions.push(AgentAction::ExecuteCommand {
                                            command: proposed_action.clone(),
                                            reasoning: "Fallback raw command execution".to_string(),
                                            confidence: 0.8,
                                        });
                                    }"""

    new_fallback = """                                    } else {
                                        let cmd_name = action_str.split_whitespace().next().unwrap_or("");
                                        let internal_tools = ["write_file", "read_file", "replace_lines", "patch_function", "diff_patch", "search_code", "read_file_chunk", "search_memory", "halt"];
                                        if internal_tools.contains(&cmd_name) {
                                            self.log(format!("\\x1b[1;31m>> [CORTEX FAULT] Intercepted malformed payload for internal tool '{}'. Preventing shell leakage.\\x1b[0m", cmd_name));
                                            let obs_id = session.graph.add_node(NodeKind::Observation, &format!("[TOOL_SYNTAX_ERROR] You attempted to use '{}' but the payload format was invalid or truncated. Ensure you provide all required arguments and formatting.", cmd_name), 1.0, 50);
                                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.25);
                                            session.graph.propagate_beliefs();
                                            continue;
                                        } else {
                                            self.log("\\x1b[33m>> [CORTEX FALLBACK] Translator produced empty JSON. Executing proposed command directly...\\x1b[0m".to_string());
                                            actions.push(AgentAction::ExecuteCommand {
                                                command: proposed_action.clone(),
                                                reasoning: "Fallback raw command execution".to_string(),
                                                confidence: 0.8,
                                            });
                                        }
                                    }"""

    if old_intercept in content and old_fallback in content:
        content = content.replace(old_intercept, new_intercept)
        content = content.replace(old_fallback, new_fallback)
        with open(path, "w") as f:
            f.write(content)
        print("[OK] Cortical Loop upgraded with Bash Leakage Guard and Native Line Editor.")
    else:
        print("[!] Target code blocks not found. May already be patched.")
else:
    print(f"[ERROR] File not found: {path}")

print(">> Upgrade Complete.")
