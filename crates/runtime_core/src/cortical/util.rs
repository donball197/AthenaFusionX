//! Pure helper functions extracted from cortical_loop.rs (no `self`, no state).

/// A `run_tests` target is interpolated into a shell command line, so only
/// a conservative character set is allowed (cargo package/test filters and
/// simple flags). Shell metacharacters (; & | $ ` ( ) < > quotes, newlines)
/// are rejected.
pub(crate) fn is_safe_test_target(target: &str) -> bool {
    target.len() <= 200
        && target.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | ':' | '.' | '/' | '=')
        })
}

pub(crate) fn compress_context(ctx: &str, max_chars: usize) -> String {
    if ctx.len() <= max_chars {
        return ctx.to_string();
    }
    let chunk_size = max_chars / 2;
    // Snap both cuts to UTF-8 char boundaries; byte-offset slicing panics
    // when a multi-byte character straddles the cut.
    let mut start_end = chunk_size;
    while !ctx.is_char_boundary(start_end) {
        start_end -= 1;
    }
    let mut end_start = ctx.len() - chunk_size;
    while !ctx.is_char_boundary(end_start) {
        end_start += 1;
    }
    let start = &ctx[..start_end];
    let end = &ctx[end_start..];
    format!("{}...\n\n[... {} CHARACTERS COMPRESSED ...]\n\n{}", start, ctx.len() - max_chars, end)
}

/// Extract the target named on the first `<marker><target>` line of a
/// mission's MANDATORY NODE SUCCESS CRITERIA block. Returns None when the
/// mission does not name a target for that marker.
///
/// Used by `action_advances_unmet_contract_condition` to keep the
/// multi-criteria safety net parameter-aware: a `READ_OK:<path>` criterion
/// must only be advanced by an action whose path matches the criterion,
/// not by any read on any file.
///
/// The marker is matched literally (e.g. "READ_OK:") and the target is
/// the first non-empty line after the marker, trimmed of whitespace and
/// of surrounding backticks or double quotes. This mirrors the
/// extraction done by EpistemicEngine::generate_hypotheses for the same
/// criteria blocks.
pub(crate) fn extract_named_target(mission: &str, marker: &str) -> Option<String> {
    let pos = mission.find(marker)?;
    let remainder = &mission[pos + marker.len()..];
    let target = remainder.lines().next()?.trim().trim_matches('`').trim_matches('"');
    if target.is_empty() {
        None
    } else {
        Some(target.to_string())
    }
}

pub(crate) fn extract_explicit_search_memory_query(mission: &str) -> Option<String> {
    let lower = mission.to_lowercase();

    let marker = "recall prior memory specifically about";
    let start = lower.find(marker)? + marker.len();

    let remainder = mission.get(start..)?.trim_start();

    let query = remainder
        .split_once('.')
        .map(|(q, _)| q)
        .unwrap_or(remainder)
        .trim()
        .trim_end_matches(':')
        .trim();

    if query.is_empty() {
        None
    } else {
        Some(query.to_string())
    }
}

/// True only for prose-only proposals that mention "halt". Typed JSON
/// proposals (containing a "type" field) that failed validation return false.
pub(crate) fn should_synthesize_halt(raw: &str) -> bool {
    let t = raw.trim();
    // JSON-shaped output (even malformed) is a parse fault, never prose: the caller must fall
    // through to parse-fault recovery. A halt-shaped object like {"halt": true, ...} used to
    // become a 28-char synthetic halt that discarded the model's real report.
    let json_shaped = t.starts_with('{')
        || t.starts_with('[')
        || t.contains("```")
        || t.contains("{\"")
        || t.contains("[{");
    t.to_lowercase().contains("halt") && !t.contains("\"type\"") && !json_shaped
}

#[cfg(test)]
mod compress_tests {
    use super::{compress_context, is_safe_test_target};

    #[test]
    fn test_target_validation() {
        for ok in [
            "",
            ".",
            "-p nexus_runtime_core",
            "-p nexus_runtime_core compress",
            "cortical::util -- --nocapture",
            "--lib",
        ] {
            assert!(is_safe_test_target(ok), "should allow {:?}", ok);
        }
        for bad in
            ["x; rm -rf ~", "a && b", "a | b", "$(id)", "`id`", "a\nb", "a > f", "'x'", "\"x\""]
        {
            assert!(!is_safe_test_target(bad), "should reject {:?}", bad);
        }
        assert!(!is_safe_test_target(&"a".repeat(201)));
    }

    #[test]
    fn ascii_behavior_unchanged() {
        let ctx = "a".repeat(100);
        let out = compress_context(&ctx, 20);
        assert!(out.starts_with(&"a".repeat(10)));
        assert!(out.ends_with(&"a".repeat(10)));
        assert!(out.contains("80 CHARACTERS COMPRESSED"));
    }

    #[test]
    fn short_input_passthrough() {
        assert_eq!(compress_context("hello", 100), "hello");
    }

    #[test]
    fn multibyte_at_cut_does_not_panic() {
        // 2-byte chars; max_chars=51 -> chunk 25 (odd) lands mid-character.
        let ctx = "é".repeat(100);
        let out = compress_context(&ctx, 51);
        assert!(out.contains("COMPRESSED"));
        // 4-byte chars and mixed widths
        let _ = compress_context(&"😀".repeat(100), 51);
        let _ = compress_context(&format!("{}{}", "a".repeat(30), "日本語".repeat(40)), 37);
    }
}

#[cfg(test)]
mod synth_halt_tests {
    use super::should_synthesize_halt;

    #[test]
    fn json_shaped_proposals_never_synthesize() {
        assert!(!should_synthesize_halt(r#"{"halt": true, "reasoning": "done"}"#));
        assert!(!should_synthesize_halt(
            "[{\"action\": \"x\", \"reasoning\": \"before halting\"}]"
        ));
    }

    #[test]
    fn prose_halt_still_synthesizes() {
        assert!(should_synthesize_halt("The task is complete, I will halt."));
    }
}

/// Normalizes a strategy string for duplicate detection: quotes dropped, whitespace
/// collapsed, ASCII-lowercased. `execute_command "x"` and `execute_command 'x'` compare equal.
pub(crate) fn normalize_strategy(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\'' | '"' | '`'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// Best-effort action name for proposals that use `action` / `tool` keys, ReAct nesting, or
/// {"halt": ...} instead of `"type"`, so a parse fault can still name the real schema fields.
pub(crate) fn fallback_action_type(raw: &str) -> Option<String> {
    for key in ["\"action\"", "\"tool\""] {
        if let Some(pos) = raw.find(key) {
            let after = raw[pos + key.len()..].trim_start();
            if let Some(rest) = after.strip_prefix(':') {
                if let Some(rest) = rest.trim_start().strip_prefix('"') {
                    if let Some(end) = rest.find('"') {
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
    }
    if raw.contains("\"halt\"") {
        return Some("halt".to_string());
    }
    // bare function-call style text such as `read_file_chunk path 1244 250`
    for name in [
        "read_file_chunk",
        "read_function",
        "read_file",
        "search_code",
        "search_memory",
        "write_file",
        "diff_patch",
        "patch_function",
        "replace_lines",
        "execute_command",
    ] {
        if raw.contains(name) {
            return Some(name.to_string());
        }
    }
    None
}
