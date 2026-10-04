//! Pure helper functions extracted from cortical_loop.rs (no `self`, no state).

/// A `run_tests` target is interpolated into a shell command line, so only
/// a conservative character set is allowed (cargo package/test filters and
/// simple flags). Shell metacharacters (; & | $ ` ( ) < > quotes, newlines)
/// are rejected.
pub(crate) fn is_safe_test_target(target: &str) -> bool {
    target.len() <= 200
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | ':' | '.' | '/' | '='))
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
    format!(
        "{}...\n\n[... {} CHARACTERS COMPRESSED ...]\n\n{}",
        start,
        ctx.len() - max_chars,
        end
    )
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
    raw.trim().to_lowercase().contains("halt") && !raw.contains("\"type\"")
}

#[cfg(test)]
mod compress_tests {
    use super::{compress_context, is_safe_test_target};

    #[test]
    fn test_target_validation() {
        for ok in ["", ".", "-p nexus_runtime_core", "-p nexus_runtime_core compress", "cortical::util -- --nocapture", "--lib"] {
            assert!(is_safe_test_target(ok), "should allow {:?}", ok);
        }
        for bad in ["x; rm -rf ~", "a && b", "a | b", "$(id)", "`id`", "a\nb", "a > f", "'x'", "\"x\""] {
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
