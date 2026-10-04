//! Pure helper functions extracted from cortical_loop.rs (no `self`, no state).

pub(crate) fn compress_context(ctx: &str, max_chars: usize) -> String {
    if ctx.len() <= max_chars {
        return ctx.to_string();
    }
    let chunk_size = max_chars / 2;
    let start = &ctx[..chunk_size];
    let end = &ctx[ctx.len() - chunk_size..];
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
