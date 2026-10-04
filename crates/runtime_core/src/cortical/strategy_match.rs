//! Deterministic strategy ↔ action compatibility checks.
//!
//! The Cortex selects the active strategy. The LLM Strategist may
//! operationalize that strategy, but may not silently substitute a
//! different native action.

use crate::agent_protocol::AgentAction;

/// Deterministically enforce the Cortex strategy/action boundary.
pub fn action_matches_strategy(strategy: &str, action: &AgentAction) -> bool {
    let strategy = strategy.trim();

    if matches!(action, AgentAction::Halt { .. }) {
        return true;
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
            AgentAction::ReadFile { path, .. } if path.trim() == expected_path
        ) || matches!(
            action,
            AgentAction::ReadFileChunk { path, .. } if path.trim() == expected_path
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
            } if path.trim() == expected_path
                && function_name.trim() == expected_function
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("read_file_chunk:") {
        return matches!(
            action,
            AgentAction::ReadFileChunk { path, .. }
                if path.trim() == expected_path.trim()
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
        return matches!(
            action,
            AgentAction::ExecuteCommand { command, .. }
                if command.trim() == expected_command.trim()
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("write_file:") {
        return matches!(
            action,
            AgentAction::WriteFile { path, .. }
                if path.trim() == expected_path.trim()
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("replace_lines:") {
        return matches!(
            action,
            AgentAction::ReplaceLines { path, .. }
                if path.trim() == expected_path.trim()
        );
    }

    if let Some(expected_path) = strategy.strip_prefix("diff_patch:") {
        return matches!(
            action,
            AgentAction::DiffPatch { path, .. }
                if path.trim() == expected_path.trim()
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
            } if path.trim() == expected_path
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
