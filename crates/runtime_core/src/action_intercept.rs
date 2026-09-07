use crate::agent_protocol::AgentAction;

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
        if parts.len() >= 2 {
            let path = parts[1].to_string();
            let query = if parts.len() >= 3 {
                parts[2..].join(" ").trim_matches('\'').trim_matches('"').to_string()
            } else {
                String::new()
            };
            return Some(AgentAction::SearchCode {
                path,
                query,
                reasoning: "Intercepted fallback search_code".to_string(),
            });
        }
    }

    if let Some(path) = action_str.strip_prefix("read_file ") {
        return Some(AgentAction::ReadFile {
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
    let lower = text.to_lowercase();

    // search_code (handles [TOOL]: search_code, Tool: search_code, etc.)
    if lower.contains("search_code") {
        let path = labeled_value(
            text,
            &["[PATH]:", "[PATH]", "Target Path:", "target path:", "Path:", "path:"],
        )
        .unwrap_or_else(|| "apps/warp".to_string());
        let query =
            labeled_value(text, &["[QUERY]:", "[QUERY]", "Query:", "query:"]).unwrap_or_default();

        return Some(AgentAction::SearchCode {
            path,
            query,
            reasoning: "Intercepted labeled search_code".to_string(),
        });
    }

    // read_file
    if lower.contains("read_file") {
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
                path,
                reasoning: "Intercepted labeled read_file".to_string(),
            });
        }
    }

    // scrape_url
    if lower.contains("scrape_url") {
        let url = labeled_value(text, &["[URL]:", "[URL]", "[SCRAPE_URL]:", "URL:", "url:"]);

        let prompt = labeled_value(
            text,
            &["[PROMPT]:", "[PROMPT]", "[EXTRACT]:", "Prompt:", "prompt:", "Extract:"],
        );

        if let (Some(url), Some(prompt)) = (url, prompt) {
            return Some(AgentAction::ScrapeUrl {
                url,
                prompt,
                reasoning: "Intercepted labeled scrape_url".to_string(),
            });
        }
    }

    // execute_command
    if lower.contains("execute_command") {
        if let Some(cmd) = labeled_value(
            text,
            &["[COMMAND]:", "[COMMAND]", "[EXECUTE_COMMAND]:", "Command:", "command:"],
        ) {
            return Some(AgentAction::ExecuteCommand {
                command: cmd.trim().trim_matches('`').to_string(),
                reasoning: "Intercepted labeled execute_command".to_string(),
                confidence: 0.85,
            });
        }
    }

    // halt
    if lower.contains("halt") || lower.contains("[halt]") {
        return Some(AgentAction::Halt {
            reasoning: "Model signaled halt via labeled proposal".to_string(),
        });
    }

    None
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
