//! Deterministic strategy ↔ action compatibility checks.
//!
//! The Cortex selects the active strategy. The LLM Strategist may
//! operationalize that strategy, but may not silently substitute a
//! different native action.

use crate::agent_protocol::AgentAction;

/// Path equivalence for strategy/action comparison. The strategist often
/// proposes an absolute path for a workspace-relative strategy
/// (`write_file:x.rs` vs `/repo/x.rs`); that is the same file and must not be
/// a STRATEGY VIOLATION. Paths containing a `..` component are never
/// normalized and match only on exact string equality.
fn same_path(actual: &str, expected: &str) -> bool {
    let a = actual.trim();
    let e = expected.trim();
    if a == e {
        return true;
    }
    let norm = |p: &str| -> Option<String> {
        if p.split('/').any(|c| c == "..") {
            return None;
        }
        let mut rest = p;
        while let Some(r) = rest.strip_prefix("./") {
            rest = r;
        }
        Some(rest.to_string())
    };
    let (Some(a), Some(e)) = (norm(a), norm(e)) else {
        return false;
    };
    if a == e {
        return true;
    }
    match std::env::current_dir() {
        Ok(cwd) => {
            let cwd = cwd.to_string_lossy().to_string();
            let cwd = cwd.trim_end_matches('/');
            let abs = |p: &str| -> String {
                if p.starts_with('/') {
                    p.to_string()
                } else {
                    format!("{}/{}", cwd, p)
                }
            };
            abs(&a) == abs(&e)
        }
        Err(_) => false,
    }
}

/// Deterministically enforce the Cortex strategy/action boundary.
pub fn action_matches_strategy(strategy: &str, action: &AgentAction) -> bool {
    let strategy = strategy.trim();

    if matches!(action, AgentAction::Halt { .. }) {
        return true;
    }

    if strategy == "halt" {
        return matches!(action, AgentAction::Halt { .. });
    }

    if strategy.starts_with("halt:") {
        return matches!(
            action,
            AgentAction::SearchCode { .. }
                | AgentAction::ReadFile { .. }
                | AgentAction::ReadFileChunk { .. }
                | AgentAction::ReadFunction { .. }
                | AgentAction::SearchMemory { .. }
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("read_file:") {
        let expected_path = expected_path.trim();
        return matches!(
            action,
            AgentAction::ReadFile { path, .. } if same_path(path, expected_path)
        ) || matches!(
            action,
            AgentAction::ReadFileChunk { path, .. } if same_path(path, expected_path)
        );
    }

    if let Some(rest) = strategy.strip_prefix("read_function:") {
        let mut parts = rest.splitn(2, ':');
        let expected_path = parts.next().unwrap_or("").trim();
        let expected_function = parts.next().unwrap_or("").trim();

        return matches!(
            action,
            AgentAction::ReadFunction {
                path,
                function_name,
                ..
            } if same_path(path, expected_path)
                && function_name.trim() == expected_function
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("read_file_chunk:") {
        return matches!(
            action,
            AgentAction::ReadFileChunk { path, .. }
                if same_path(path, expected_path)
        );
    }

    if let Some(rest) = strategy.strip_prefix("search_code:") {
        let mut parts = rest.splitn(2, ':');
        let first = parts.next().unwrap_or("").trim();
        let second = parts.next().map(|s| s.trim());

        return match second {
            Some(expected_query) => {
                let expected_path = first;
                let expected_path_norm = expected_path.trim_end_matches('/');
                let prefix = format!("{}/", expected_path_norm);

                let actual_q = if let AgentAction::SearchCode { query, .. } = action {
                    query.trim().to_lowercase()
                } else {
                    String::new()
                };
                let expected_q = expected_query.trim().to_lowercase();
                let min_len = actual_q.len().min(expected_q.len());
                let query_matches = actual_q == expected_q
                    || (min_len >= 4
                        && (actual_q.contains(&expected_q) || expected_q.contains(&actual_q)));

                let path_ok = |p: &str| -> bool {
                    let pt = p.trim();
                    pt == expected_path
                        || (!expected_path_norm.is_empty()
                            && pt.trim_end_matches('/') == expected_path_norm)
                        || (pt.starts_with(&prefix) && !pt.contains(".."))
                        || pt.ends_with(&format!("/{}", expected_path_norm))
                        || (pt.contains(&format!("/{}/", expected_path_norm)) && !pt.contains(".."))
                };

                match action {
                    AgentAction::SearchCode { path, .. } => path_ok(path) && query_matches,
                    AgentAction::ReadFileChunk { path, .. } => path_ok(path),
                    AgentAction::ReadFile { path, .. } => path_ok(path),
                    AgentAction::ExecuteCommand { command, .. } => {
                        let cmd = command.trim();
                        let is_inspection_cmd = ["ls ", "ls -", "stat ", "test ", "file "]
                            .iter()
                            .any(|verb| cmd.starts_with(verb));
                        let expected_query_lower = expected_query.trim().to_lowercase();
                        is_inspection_cmd
                            && (cmd.contains(expected_path)
                                || cmd.to_lowercase().contains(&expected_query_lower))
                    }
                    _ => false,
                }
            }
            None => {
                let expected_query = first;
                matches!(
                    action,
                    AgentAction::SearchCode { query, .. } if query.trim() == expected_query
                )
            }
        };
    }

    if strategy.strip_prefix("search_memory:").is_some() {
        return matches!(action, AgentAction::SearchMemory { .. });
    }

    if let Some(expected_command) = strategy.strip_prefix("execute_command:") {
        let expected = crate::cmd_match::normalize_cmd(expected_command);
        return matches!(
            action,
            AgentAction::ExecuteCommand { command, .. }
                if crate::cmd_match::normalize_cmd(command) == expected
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("write_file:") {
        return matches!(
            action,
            AgentAction::WriteFile { path, .. }
                if same_path(path, expected_path)
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("replace_lines:") {
        return matches!(
            action,
            AgentAction::ReplaceLines { path, .. }
                if same_path(path, expected_path)
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("diff_patch:") {
        return matches!(
            action,
            AgentAction::DiffPatch { path, .. }
                if same_path(path, expected_path)
        );
    }

    if let Some(rest) = strategy.strip_prefix("patch_function:") {
        let mut parts = rest.splitn(2, ':');
        let expected_path = parts.next().unwrap_or("").trim();
        let _expected_function = parts.next().unwrap_or("").trim();

        return matches!(
            action,
            AgentAction::PatchFunction {
                path,
                function_name,
                ..
            } if same_path(path, expected_path)
                && !function_name.trim().is_empty()
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("synthesize_report:") {
        return matches!(
            action,
            AgentAction::SynthesizeReport { output_path, .. }
                if output_path.trim() == expected_path.trim()
        );
    }

    true
}

/// Return true if `action_path` is still a mandatory unread target.
/// Callers pass paths from the node contract that are not yet fully covered.
pub fn is_mandatory_unread_path(action_path: &str, mandatory_unread: &[String]) -> bool {
    mandatory_unread.iter().any(|m| {
        let a = action_path.trim_start_matches("./");
        let b = m.trim_start_matches("./");
        a == b || a.ends_with(b) || b.ends_with(a)
    })
}

/// STRATEGY_VIOLATION escape hatch: path differs from active strategy, but the
/// action targets a still-mandatory unread file for this node.
pub fn allow_mandatory_pivot(
    strategy_path: &str,
    action_path: &str,
    mandatory_unread: &[String],
) -> bool {
    if strategy_path == action_path {
        return true;
    }
    is_mandatory_unread_path(action_path, mandatory_unread)
}

#[cfg(test)]
mod same_path_tests {
    use super::same_path;

    fn cwd() -> String {
        std::env::current_dir().unwrap().to_string_lossy().trim_end_matches('/').to_string()
    }

    #[test]
    fn identical_and_dot_slash_paths_match() {
        assert!(same_path("x.rs", "x.rs"));
        assert!(same_path("./x.rs", "x.rs"));
        assert!(same_path(" x.rs ", "x.rs"));
    }

    #[test]
    fn absolute_path_under_cwd_matches_relative() {
        let abs = format!("{}/x.rs", cwd());
        assert!(same_path(&abs, "x.rs"));
        assert!(same_path("x.rs", &abs));
    }

    #[test]
    fn absolute_path_elsewhere_does_not_match() {
        assert!(!same_path("/etc/x.rs", "x.rs"));
        assert!(!same_path(&format!("{}/other/x.rs", cwd()), "x.rs"));
    }

    #[test]
    fn dotdot_components_are_never_normalized() {
        assert!(!same_path("../x.rs", "x.rs"));
        assert!(!same_path(&format!("{}/../x.rs", cwd()), "x.rs"));
        assert!(same_path("../x.rs", "../x.rs"));
    }

    #[test]
    fn different_files_do_not_match() {
        assert!(!same_path("a.rs", "b.rs"));
    }
}

#[cfg(test)]
mod action_matches_strategy_tests {
    use super::action_matches_strategy;
    use crate::agent_protocol::AgentAction;
    use crate::json_parser::extract_actions;

    fn act(json: &str) -> AgentAction {
        extract_actions(json)
            .expect("test payload should parse")
            .into_iter()
            .next()
            .expect("expected exactly one action")
    }

    // Regression: patch_function against replace_lines strategy.
    // This is the exact mismatch that let the OpenFang mutation land.
    #[test]
    fn patch_function_against_replace_lines_strategy_is_rejected() {
        let action = act(
            r#"[{"type":"patch_function","path":"src/lib.rs","function_name":"new","new_body":"fn new() {}","reasoning":"test"}]"#,
        );
        assert!(
            !action_matches_strategy("replace_lines:src/lib.rs", &action),
            "patch_function must not satisfy a replace_lines strategy"
        );
    }

    #[test]
    fn replace_lines_against_replace_lines_strategy_is_accepted() {
        let action = act(
            r#"[{"type":"replace_lines","path":"src/lib.rs","start_line":1,"end_line":2,"replacement":"x","reasoning":"test"}]"#,
        );
        assert!(
            action_matches_strategy("replace_lines:src/lib.rs", &action),
            "replace_lines must satisfy a replace_lines strategy on the same path"
        );
    }

    #[test]
    fn replace_lines_against_wrong_path_is_rejected() {
        let action = act(
            r#"[{"type":"replace_lines","path":"other.rs","start_line":1,"end_line":2,"replacement":"x","reasoning":"test"}]"#,
        );
        assert!(
            !action_matches_strategy("replace_lines:src/lib.rs", &action),
            "replace_lines against a different path must be rejected"
        );
    }

    #[test]
    fn patch_function_against_patch_function_strategy_is_accepted() {
        let action = act(
            r#"[{"type":"patch_function","path":"src/lib.rs","function_name":"new","new_body":"fn new() {}","reasoning":"test"}]"#,
        );
        assert!(
            action_matches_strategy("patch_function:src/lib.rs:new", &action),
            "patch_function must satisfy a patch_function strategy on the same path"
        );
    }

    #[test]
    fn write_file_against_write_file_strategy_is_accepted() {
        let action =
            act(r#"[{"type":"write_file","path":"out.md","content":"hello","reasoning":"test"}]"#);
        assert!(action_matches_strategy("write_file:out.md", &action));
    }

    #[test]
    fn search_scope_with_trailing_slash_matches_bare_and_absolute_paths() {
        let bare = act(r#"[{"type":"search_code","path":"bins","query":"main","reasoning":"t"}]"#);
        let slash =
            act(r#"[{"type":"search_code","path":"bins/","query":"main","reasoning":"t"}]"#);
        let abs =
            act(r#"[{"type":"search_code","path":"/root/X/bins","query":"main","reasoning":"t"}]"#);
        assert!(action_matches_strategy("search_code:bins/:main", &bare));
        assert!(action_matches_strategy("search_code:bins/:main", &slash));
        assert!(action_matches_strategy("search_code:bins/:main", &abs));
        assert!(action_matches_strategy("search_code:bins:main", &bare));
    }

    #[test]
    fn search_scope_still_rejects_other_directories_and_queries() {
        let other =
            act(r#"[{"type":"search_code","path":"/root/X/apps","query":"main","reasoning":"t"}]"#);
        let lookalike =
            act(r#"[{"type":"search_code","path":"binsx","query":"main","reasoning":"t"}]"#);
        let wrong_q =
            act(r#"[{"type":"search_code","path":"bins","query":"unrelated","reasoning":"t"}]"#);
        assert!(!action_matches_strategy("search_code:bins/:main", &other));
        assert!(!action_matches_strategy("search_code:bins/:main", &lookalike));
        assert!(!action_matches_strategy("search_code:bins/:main", &wrong_q));
    }

    #[test]
    fn execute_command_with_report_suffix_matches_bare_action() {
        // Submission 1 failure: strategy carried " and report stdout."
        // from the directive, but the action's command was the bare shell
        // command. Contract check passed but strategy check failed, deadlocking
        // the cortex. Both sides must normalize the same way.
        let action =
            act(r#"[{"type":"execute_command","command":"find X | wc -l","reasoning":"test"}]"#);
        assert!(action_matches_strategy(
            "execute_command:find X | wc -l and report stdout.",
            &action
        ));
    }
}
