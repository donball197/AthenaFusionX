use crate::agent_protocol::AgentAction;

/// When the Translator LLM produces an empty action list, this parses the
/// Strategist's raw proposed-action text directly as a fallback. Pure string
/// parsing -- no side effects, no borrows of CorticalLoop state -- which is
/// exactly why it's the first piece pulled out of the ~500-line
/// ignite_with_evidence function in cortical_loop.rs.
///
/// Returns None if the text doesn't match any known tool prefix or labeled
/// multi-line form; the caller falls back further to raw ExecuteCommand
/// (see cortical_loop.rs).
///
/// NOTE: deliberately does NOT include a write_file branch. That branch was
/// removed from cortical_loop.rs earlier this session (see [CORTEX SAFETY]
/// log line) because raw, un-JSON-escaped Strategist text written straight
/// to disk caused real file corruption. Do not re-add it here.
pub fn intercept_fallback_action(action_str: &str) -> Option<AgentAction> {
    let action_str = action_str.trim();

    // --- Prefix form (translator / single-line) ---
    if let Some(cmd) = action_str.strip_prefix("execute_command") {
        let clean_cmd = cmd.trim().trim_matches('\'').trim_matches('"').to_string();
        return Some(AgentAction::ExecuteCommand {
            command: clean_cmd,
            reasoning: "Intercepted fallback execute_command".to_string(),
            confidence: 0.9,
        });
    }
    if action_str.starts_with("search_code") {
        let parts: Vec<&str> = action_str.split_whitespace().collect();
        if parts.len() >= 3 {
            let path = parts[1].to_string();
            let query = parts[2..].join(" ").trim_matches('\'').trim_matches('"').to_string();
            return Some(AgentAction::SearchCode {
                path,
                query,
                reasoning: "Intercepted fallback search_code".to_string(),
            });
        }
        return None;
    }
    if action_str.starts_with("read_file_chunk") {
        let parts: Vec<&str> = action_str.split_whitespace().collect();
        if parts.len() >= 2 {
            let path = parts[1].to_string();
            let start_line = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let max_lines = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(500);
            return Some(AgentAction::ReadFileChunk {
                path,
                start_line,
                max_lines,
                reasoning: "Intercepted fallback read_file_chunk".to_string(),
            });
        }
        return None;
    }
    if let Some(stripped) = action_str.strip_prefix("diff_patch ") {
        let parts: Vec<&str> = stripped.split_whitespace().collect();
        if parts.len() >= 3 {
            let path = parts[0].to_string();
            let search = parts[1].to_string();
            let replace = parts[2..].join(" ");
            return Some(AgentAction::DiffPatch {
                path,
                search,
                replace,
                reasoning: "Intercepted fallback diff_patch".to_string(),
            });
        }
        return None;
    }
    if let Some(stripped) = action_str.strip_prefix("patch_function ") {
        let parts: Vec<&str> = stripped.split_whitespace().collect();
        if parts.len() >= 3 {
            let path = parts[0].to_string();
            let function_name = parts[1].to_string();
            let new_body = parts[2..].join(" ");
            return Some(AgentAction::PatchFunction {
                path,
                function_name,
                new_body,
                reasoning: "Intercepted fallback patch_function".to_string(),
            });
        }
        return None;
    }
    if let Some(path) = action_str.strip_prefix("read_file ") {
        return Some(AgentAction::ReadFile {
            path: path.trim().to_string(),
            reasoning: "Intercepted fallback read_file".to_string(),
        });
    }
    if let Some(raw_query) = action_str.strip_prefix("search_memory") {
        let query = raw_query.trim().trim_matches('\'').trim_matches('"').to_string();
        return Some(AgentAction::SearchMemory {
            query,
            reasoning: "Intercepted fallback search_memory".to_string(),
        });
    }

    // --- Labeled multi-line form (Strategist NL) ---
    // Matches proposals like:
    //   Tool: read_file
    //   Target Path: /tmp/openfang/SECURITY.md
    // Does not handle write_file (same safety contract as above).
    if let Some(action) = intercept_labeled_proposal(action_str) {
        return Some(action);
    }

    None
}

/// Strategist-style labeled proposals (Tool: / Target Path: / Command: / Query:).
fn intercept_labeled_proposal(text: &str) -> Option<AgentAction> {
    let lower = text.to_lowercase();

    // read_file
    if lower.contains("tool: read_file")
        || lower.contains("use the `read_file`")
        || lower.contains("use the read_file")
    {
        if let Some(path) =
            labeled_value(text, &["Target Path:", "target path:", "Path:", "path:"])
        {
            if looks_like_path(&path) {
                return Some(AgentAction::ReadFile {
                    path,
                    reasoning: "Intercepted labeled read_file".to_string(),
                });
            }
        }
    }

    // search_code
    if lower.contains("tool: search_code")
        || lower.contains("use the `search_code`")
        || lower.contains("use the search_code")
    {
        let path = labeled_value(text, &["Path:", "path:"]);
        let query = labeled_value(text, &["Query:", "query:"]);
        if let (Some(path), Some(query)) = (path, query) {
            if looks_like_path(&path) && !query.is_empty() {
                return Some(AgentAction::SearchCode {
                    path,
                    query,
                    reasoning: "Intercepted labeled search_code".to_string(),
                });
            }
        }
    }

    // execute_command — require Tool/Command labels, not bare "git clone" anywhere
    if lower.contains("tool: execute_command")
        || lower.contains("use the execute_command")
        || lower.contains("use the `execute_command`")
    {
        if let Some(cmd) = labeled_value(text, &["Command:", "command:"]) {
            let cmd = cmd.trim().trim_matches('`').to_string();
            if !cmd.is_empty() && !cmd.to_lowercase().starts_with("tool") {
                return Some(AgentAction::ExecuteCommand {
                    command: cmd,
                    reasoning: "Intercepted labeled execute_command".to_string(),
                    confidence: 0.85,
                });
            }
        }
    }

    None
}

fn labeled_value(text: &str, markers: &[&str]) -> Option<String> {
    for marker in markers {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.len() >= marker.len()
                && trimmed[..marker.len()].eq_ignore_ascii_case(marker)
            {
                let rest = trimmed[marker.len()..]
                    .trim()
                    .trim_matches('`')
                    .trim_matches('"');
                if !rest.is_empty() {
                    return Some(rest.to_string());
                }
            }
        }
    }
    None
}

fn looks_like_path(p: &str) -> bool {
    p.starts_with('/')
        || p.starts_with("./")
        || p.ends_with(".rs")
        || p.ends_with(".toml")
        || p.ends_with(".md")
        || p.ends_with(".json")
        || p.contains('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_code() {
        match intercept_fallback_action("search_code apps/foo.rs api_key") {
            Some(AgentAction::SearchCode { path, query, .. }) => {
                assert_eq!(path, "apps/foo.rs");
                assert_eq!(query, "api_key");
            }
            other => panic!("expected SearchCode, got {:?}", other),
        }
    }

    #[test]
    fn no_write_file_branch() {
        assert!(intercept_fallback_action("write_file foo.rs bar").is_none());
    }

    #[test]
    fn unrecognized_returns_none() {
        assert!(intercept_fallback_action("halt").is_none());
    }

    #[test]
    fn labeled_read_file() {
        let text = "I will use the `read_file` tool.\n\
                    Tool: read_file\n\
                    Target Path: /tmp/openfang/SECURITY.md\n";
        match intercept_fallback_action(text) {
            Some(AgentAction::ReadFile { path, .. }) => {
                assert_eq!(path, "/tmp/openfang/SECURITY.md");
            }
            other => panic!("expected ReadFile, got {:?}", other),
        }
    }

    #[test]
    fn labeled_execute_command() {
        let text = "Tool: execute_command\nCommand: ls -R /tmp/openfang\n";
        match intercept_fallback_action(text) {
            Some(AgentAction::ExecuteCommand { command, .. }) => {
                assert_eq!(command, "ls -R /tmp/openfang");
            }
            other => panic!("expected ExecuteCommand, got {:?}", other),
        }
    }

    #[test]
    fn labeled_search_code() {
        let text = "Tool: search_code\nPath: /tmp/openfang/crates/openfang-memory\nQuery: sqlite\n";
        match intercept_fallback_action(text) {
            Some(AgentAction::SearchCode { path, query, .. }) => {
                assert_eq!(path, "/tmp/openfang/crates/openfang-memory");
                assert_eq!(query, "sqlite");
            }
            other => panic!("expected SearchCode, got {:?}", other),
        }
    }

    #[test]
    fn labeled_still_rejects_write_file() {
        let text = "Tool: write_file\nPath: foo.rs\nContent: evil\n";
        assert!(intercept_fallback_action(text).is_none());
    }

    #[test]
    fn bare_git_clone_without_tool_label_is_not_intercepted() {
        // Must not match loose substrings — only labeled Tool/Command form.
        assert!(intercept_fallback_action("git clone --depth 1 https://example.com/repo.git").is_none());
    }
}
