use crate::agent_protocol::AgentAction;

pub fn intercept_fallback_action(action_str: &str) -> Option<AgentAction> {
    let action_str = action_str.trim();

    if let Some(cmd) = action_str.strip_prefix("execute_command") {
        let clean_cmd = cmd.trim().trim_matches('\'').trim_matches('"').to_string();
        return Some(AgentAction::ExecuteCommand {
            _thought_process: None,
            command: clean_cmd,
            reasoning: "Intercepted fallback execute_command".to_string(),
            confidence: 0.9,
        });
    }

    if action_str.starts_with("search_code ") || action_str == "search_code" {
        let parts: Vec<&str> = action_str.split_whitespace().collect();
        if parts.len() >= 3 && parts[0] == "search_code" {
            let path = parts[1].to_string();
            let query =
                parts[2..].join(" ").trim_matches('\'').trim_matches('"').to_string();

            if !path.trim().is_empty() && !query.trim().is_empty() {
                return Some(AgentAction::SearchCode {
                    _thought_process: None,
                    path,
                    query,
                    reasoning: "Intercepted fallback search_code".to_string(),
                });
            }
        }
    }

    if let Some(path) = action_str.strip_prefix("read_file ") {
        return Some(AgentAction::ReadFile {
            _thought_process: None,
            path: path.trim().trim_matches('`').trim_matches('"').to_string(),
            reasoning: "Intercepted fallback read_file".to_string(),
        });
    }

    if let Some(rest) = action_str.strip_prefix("scrape_url") {
        let rest = rest.trim();

        let mut parts = rest.splitn(2, char::is_whitespace);
        let url = parts.next().unwrap_or("").trim().trim_matches('`').trim_matches('"').to_string();

        let prompt =
            parts.next().unwrap_or("").trim().trim_matches('`').trim_matches('"').to_string();

        if !url.is_empty() && !prompt.is_empty() {
            return Some(AgentAction::ScrapeUrl {
                _thought_process: None,
                url,
                prompt,
                reasoning: "Intercepted fallback scrape_url".to_string(),
            });
        }
    }

    if let Some(action) = intercept_labeled_proposal(action_str) {
        return Some(action);
    }

    None
}

fn intercept_labeled_proposal(text: &str) -> Option<AgentAction> {
    let _lower = text.to_lowercase();

    // search_code: only recognize an explicit action label, never a mere
    // mention of the tool name in natural-language reasoning.
    if has_action_label(text, &["[TOOL]:", "Tool:", "[ACTION]:", "Action:"])
        && lower_action_label(text).contains("search_code")
    {
        let path = labeled_value(
            text,
            &["[PATH]:", "[PATH]", "Target Path:", "target path:", "Path:", "path:"],
        );
        let query =
            labeled_value(text, &["[QUERY]:", "[QUERY]", "Query:", "query:"]);

        let (Some(path), Some(query)) = (path, query) else {
            return None;
        };
        if path.trim().is_empty() || query.trim().is_empty() {
            return None;
        }

        return Some(AgentAction::SearchCode {
            _thought_process: None,
            path,
            query,
            reasoning: "Intercepted labeled search_code".to_string(),
        });
    }

    // read_file: require an explicit action label.
    if has_action_label(text, &["[TOOL]:", "Tool:", "[ACTION]:", "Action:"])
        && lower_action_label(text).contains("read_file")
    {
        if let Some(path) = labeled_value(
            text,
            &[
                "[PATH]:",
                "[PATH]",
                "[READ_FILE_TARGET]:",
                "[READ_FILE]:",
                "Target Path:",
                "Path:",
                "path:",
            ],
        ) {
            return Some(AgentAction::ReadFile {
                _thought_process: None,
                path,
                reasoning: "Intercepted labeled read_file".to_string(),
            });
        }
    }

    // scrape_url: require an explicit action label.
    if has_action_label(text, &["[TOOL]:", "Tool:", "[ACTION]:", "Action:"])
        && lower_action_label(text).contains("scrape_url")
    {
        let url = labeled_value(text, &["[URL]:", "[URL]", "[SCRAPE_URL]:", "URL:", "url:"]);

        let prompt = labeled_value(
            text,
            &["[PROMPT]:", "[PROMPT]", "[EXTRACT]:", "Prompt:", "prompt:", "Extract:"],
        );

        if let (Some(url), Some(prompt)) = (url, prompt) {
            return Some(AgentAction::ScrapeUrl {
                _thought_process: None,
                url,
                prompt,
                reasoning: "Intercepted labeled scrape_url".to_string(),
            });
        }
    }

    // execute_command: require an explicit action label.
    if has_action_label(text, &["[TOOL]:", "Tool:", "[ACTION]:", "Action:"])
        && lower_action_label(text).contains("execute_command")
    {
        if let Some(cmd) = labeled_value(
            text,
            &["[COMMAND]:", "[COMMAND]", "[EXECUTE_COMMAND]:", "Command:", "command:"],
        ) {
            return Some(AgentAction::ExecuteCommand {
                _thought_process: None,
                command: cmd.trim().trim_matches('`').to_string(),
                reasoning: "Intercepted labeled execute_command".to_string(),
                confidence: 0.85,
            });
        }
    }

    // Halt is intentionally NOT inferred from arbitrary prose. A model
    // mentioning "halt" is not the same thing as proposing a Halt action.
    // The strict JSON parser is authoritative; this interceptor may only
    // recover an explicitly labeled action.
    if has_action_label(text, &["[TOOL]:", "Tool:", "[ACTION]:", "Action:"])
        && lower_action_label(text).trim() == "halt"
    {
        return Some(AgentAction::Halt {
            _thought_process: None,
            reasoning: "Model explicitly labeled a halt action".to_string(),
        });
    }

    None
}

fn has_action_label(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| {
        text.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.len() >= marker.len() && trimmed[..marker.len()].eq_ignore_ascii_case(marker)
        })
    })
}

fn lower_action_label(text: &str) -> String {
    for marker in ["[TOOL]:", "Tool:", "[ACTION]:", "Action:"] {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.len() >= marker.len() && trimmed[..marker.len()].eq_ignore_ascii_case(marker)
            {
                return trimmed[marker.len()..].trim().to_lowercase();
            }
        }
    }
    String::new()
}

fn labeled_value(text: &str, markers: &[&str]) -> Option<String> {
    for marker in markers {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.len() >= marker.len() && trimmed[..marker.len()].eq_ignore_ascii_case(marker)
            {
                let rest = trimmed[marker.len()..]
                    .trim()
                    .trim_matches('`')
                    .trim_matches('"')
                    .trim_matches(':');
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::intercept_fallback_action;
    use crate::agent_protocol::AgentAction;

    #[test]
    fn rejects_natural_language_mentions_of_actions() {
        assert!(intercept_fallback_action(
            "The previous strategy suggested execute_command cargo test, but this is only analysis."
        ).is_none());

        assert!(intercept_fallback_action("The mission is incomplete. Do not halt yet.").is_none());

        assert!(intercept_fallback_action(
            "We should use search_code to inspect the implementation."
        )
        .is_none());

        assert!(
            intercept_fallback_action("I need to read_file before making a conclusion.").is_none()
        );
    }

    #[test]
    fn accepts_explicit_labeled_halt() {
        assert!(matches!(
            intercept_fallback_action("[ACTION]: halt"),
            Some(AgentAction::Halt { _thought_process: None, .. })
        ));
    }

    #[test]
    fn accepts_explicit_labeled_execute_command() {
        assert!(matches!(
            intercept_fallback_action("[TOOL]: execute_command
        [COMMAND]: cargo test"),
            Some(AgentAction::ExecuteCommand { _thought_process: None, command, .. })
                if command == "cargo test"
        ));
    }

    #[test]
    fn rejects_labeled_search_code_without_path() {
        assert!(intercept_fallback_action(
            "[ACTION]: search_code\n[QUERY]: router"
        ).is_none());
    }

    #[test]
    fn rejects_labeled_search_code_without_query() {
        assert!(intercept_fallback_action(
            "[ACTION]: search_code\n[PATH]: crates/runtime_core/src\n[QUERY]:"
        ).is_none());
    }

    #[test]
    fn accepts_labeled_search_code_with_path_and_query() {
        assert!(matches!(
            intercept_fallback_action(
                "[ACTION]: search_code\n[PATH]: crates/runtime_core/src\n[QUERY]: router"
            ),
            Some(AgentAction::SearchCode { path, query, .. })
                if path == "crates/runtime_core/src" && query == "router"
        ));
    }

    #[test]
    fn rejects_free_form_search_code_without_path() {
        assert!(intercept_fallback_action("search_code").is_none());
    }

    #[test]
    fn rejects_free_form_search_code_without_query() {
        assert!(intercept_fallback_action("search_code crates/runtime_core/src").is_none());
    }

    #[test]
    fn accepts_valid_free_form_search_code() {
        assert!(matches!(
            intercept_fallback_action("search_code crates/runtime_core/src router"),
            Some(AgentAction::SearchCode { path, query, .. })
                if path == "crates/runtime_core/src" && query == "router"
        ));
    }
}
