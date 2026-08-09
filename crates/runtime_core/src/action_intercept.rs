use crate::agent_protocol::AgentAction;

/// When the Translator LLM produces an empty action list, this parses the
/// Strategist's raw proposed-action text directly as a fallback. Pure string
/// parsing -- no side effects, no borrows of CorticalLoop state -- which is
/// exactly why it's the first piece pulled out of the ~500-line
/// ignite_with_evidence function in cortical_loop.rs.
///
/// Returns None if the text doesn't match any known tool prefix; the caller
/// falls back further to raw ExecuteCommand (see cortical_loop.rs).
///
/// NOTE: deliberately does NOT include a write_file branch. That branch was
/// removed from cortical_loop.rs earlier this session (see [CORTEX SAFETY]
/// log line) because raw, un-JSON-escaped Strategist text written straight
/// to disk caused real file corruption. Do not re-add it here.
pub fn intercept_fallback_action(action_str: &str) -> Option<AgentAction> {
    let action_str = action_str.trim();

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

    None
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
        // write_file must NOT be handled here -- deliberately excluded after
        // the corruption incident. Should fall through to None.
        assert!(intercept_fallback_action("write_file foo.rs bar").is_none());
    }

    #[test]
    fn unrecognized_returns_none() {
        assert!(intercept_fallback_action("halt").is_none());
    }
}
