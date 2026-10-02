#![allow(clippy::items_after_test_module)]
//! Shared marker predicates over the free-form evidence log.
//! Keep local_evidence as String; query it through these helpers only.

/// Successful substrate mutation markers produced by the executor.
pub fn has_mutation_marker(evidence: &str) -> bool {
    evidence.lines().any(|l| {
        l.starts_with("[WRITE_OK]")
            || l.starts_with("[PATCH_FUNCTION_OK]")
            || l.starts_with("[REPLACE_LINES_OK]")
            || l.starts_with("[DIFF_PATCH_OK]")
    })
}

/// Successful recon / read / search / command markers.
/// True when `evidence` contains a [SEARCH_CODE_OK] block whose recorded
/// `Query:` field satisfies the `path:term` (or bare `term`) expectation.
///
/// The searched-for term lives in the `Query:` field of the marker header,
/// not in the lines below `MATCHES:`. The executor records the query in the
/// header and may truncate the result body. Matching is scoped to one marker
/// block so an unrelated inherited search cannot satisfy this criterion.
pub fn search_term_satisfied(evidence: &str, expected: &str) -> bool {
    let expected = expected.trim().to_lowercase();
    let (path_part, term_part) = match expected.split_once(':') {
        Some((p, t)) => (p.trim().to_string(), t.trim().to_string()),
        None => (String::new(), expected.clone()),
    };

    if term_part.is_empty() {
        return false;
    }

    evidence.split("[SEARCH_CODE_OK]").skip(1).any(|block| {
        // A search that explicitly reports no matches is not a satisfied
        // search criterion. Keep this decision local to this marker block.
        if !block.contains("MATCHES:") {
            return false;
        }

        let header = block.lines().next().unwrap_or("").trim();

        let Some((path_field, query_field)) = header.split_once("| Query:") else {
            return false;
        };

        let Some(recorded_path) = path_field.strip_prefix("Path:") else {
            return false;
        };

        let recorded_path = recorded_path.trim().to_lowercase();
        let recorded_query =
            query_field.trim().trim_matches(|c| c == '\'' || c == '"' || c == '`').to_lowercase();

        if recorded_query.is_empty() {
            return false;
        }

        let path_ok = path_part.is_empty()
            || recorded_path == path_part
            || recorded_path.ends_with(&format!("/{}", path_part));

        path_ok && recorded_query.contains(&term_part)
    })
}

pub fn has_recon_marker(evidence: &str) -> bool {
    evidence.lines().any(|l| {
        l.starts_with("[READ_OK]")
            || l.starts_with("[READ_FUNCTION_OK]")
            || l.starts_with("[READ_CHUNK_OK]")
            || l.starts_with("[SEARCH_CODE_OK]")
            || l.starts_with("[CMD_OK]")
    })
}

/// Returns true when a PATCH_FUNCTION contract has been physically satisfied.
///
/// PATCH_IDEMPOTENT_OK means the requested transformation was already present,
/// so it satisfies the patch contract without representing a new mutation.
pub fn has_patch_satisfied_marker(evidence: &str) -> bool {
    evidence.lines().any(|l| {
        l.starts_with("[PATCH_FUNCTION_OK]")
            || l.starts_with("[PATCH_IDEMPOTENT_OK]")
            || l.starts_with("[DIFF_PATCH_OK]")
            || l.starts_with("[REPLACE_LINES_OK]")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_detects_write_and_patch() {
        assert!(has_mutation_marker("[WRITE_OK] foo.rs\n"));
        assert!(has_mutation_marker("[PATCH_FUNCTION_OK] Updated function 'x' in a.rs\n"));
        assert!(!has_mutation_marker("[READ_OK] foo.rs\n"));
    }

    #[test]
    fn recon_detects_read_search_cmd() {
        assert!(has_recon_marker("[READ_OK] a\n"));
        assert!(has_recon_marker("[SEARCH_CODE_OK] Path: crates | Query: 'x'\n"));
        assert!(has_recon_marker("[CMD_OK] Status: 0\n"));
        assert!(!has_recon_marker("[WRITE_OK] a\n"));
    }

    #[test]
    fn patch_satisfied_detects_mutation_and_idempotent_markers() {
        assert!(has_patch_satisfied_marker("[PATCH_FUNCTION_OK] Updated function 'x' in a.rs\n"));
        assert!(has_patch_satisfied_marker("[PATCH_IDEMPOTENT_OK] a.rs:x\n"));
        assert!(has_patch_satisfied_marker("[DIFF_PATCH_OK] a.rs\n"));
        assert!(has_patch_satisfied_marker("[REPLACE_LINES_OK] a.rs\n"));
        assert!(!has_patch_satisfied_marker("[READ_OK] a.rs\n"));
    }

    #[test]
    fn idempotent_marker_is_not_generic_mutation() {
        assert!(!has_mutation_marker("[PATCH_IDEMPOTENT_OK] a.rs:x\n"));
    }

    #[test]
    fn search_term_matches_recorded_query_not_body() {
        let ev = "[SEARCH_CODE_OK] Path: src | Query: some_term\nMATCHES:\nline 1\n";
        assert!(search_term_satisfied(ev, "some_term"));
    }

    #[test]
    fn search_term_matches_quoted_query_with_path_scope() {
        let ev =
            "[SEARCH_CODE_OK] Path: vendor/openfang | Query: 'Cargo.toml'\nMATCHES:\nworkspace member listing\n";
        assert!(search_term_satisfied(ev, "vendor/openfang:Cargo.toml"));
    }

    #[test]
    fn search_term_survives_truncated_match_body() {
        let ev =
            "[SEARCH_CODE_OK] Path: src | Query: 'needle'\nMATCHES:\nlong output...\n[MATCHES TRUNCATED: OUTPUT TOO LARGE]\n";
        assert!(search_term_satisfied(ev, "needle"));
    }

    #[test]
    fn search_term_rejects_inherited_unrelated_marker() {
        let ev =
            "[SEARCH_CODE_OK] Path: src | Query: other_term\nMATCHES:\ntarget_term only in body\n";
        assert!(!search_term_satisfied(ev, "target_term"));
    }

    #[test]
    fn search_term_rejects_documented_empty_result() {
        let ev =
            "[SEARCH_CODE_OK] Path: src | Query: 'target'\nNo code matches found in specified path.";
        assert!(!search_term_satisfied(ev, "target"));
    }

    #[test]
    fn artifact_write_matches_by_full_path_and_basename() {
        let ev = "[WRITE_OK] crates/runtime_core/src/foo.rs\n";
        assert!(has_artifact_write(ev, "crates/runtime_core/src/foo.rs"));
        assert!(has_artifact_write(ev, "src/foo.rs"));
        assert!(has_artifact_write(ev, "foo.rs"));
        assert!(has_artifact_write(ev, "./foo.rs"));
    }

    #[test]
    fn artifact_write_rejects_same_basename_wrong_path() {
        let ev = "[WRITE_OK] crates/other/src/foo.rs\n";
        assert!(!has_artifact_write(ev, "crates/runtime_core/src/foo.rs"));
        // absolute-path tolerance is retained via the `/`-suffix rule
        let abs = "[WRITE_OK] /home/x/AthenaFusionX/crates/runtime_core/src/foo.rs\n";
        assert!(has_artifact_write(abs, "crates/runtime_core/src/foo.rs"));
    }

    #[test]
    fn artifact_write_rejects_substring_only_matches() {
        assert!(!has_artifact_write("[WRITE_OK] src/foobar.rs\n", "bar.rs"));
        assert!(!has_artifact_write("[WRITE_OK] final_report.md\n", "report.md"));
        assert!(!has_artifact_write("[WRITE_OK] myfoo.rs\n", "foo.rs"));
    }

    #[test]
    fn artifact_write_matches_synthesized_report_marker() {
        let ev = "[WRITE_OK] Successfully synthesized and wrote report to out/report.md\n";
        assert!(has_artifact_write(ev, "report.md"));
        assert!(has_artifact_write(ev, "out/report.md"));
        assert!(!has_artifact_write(ev, "port.md"));
    }

    #[test]
    fn artifact_write_tolerates_trailing_punctuation() {
        let ev = "[WRITE_OK] Successfully synthesized and wrote report to out/report.md.\n";
        assert!(has_artifact_write(ev, "report.md"));
    }

    #[test]
    fn artifact_write_rejects_non_write_markers() {
        assert!(!has_artifact_write("[READ_OK] foo.rs\n", "foo.rs"));
        assert!(!has_artifact_write("[SEARCH_CODE_OK] Path: foo.rs | Query: 'x'\n", "foo.rs"));
    }
}

/// PATCH 1a: Returns true if `evidence` contains a successful write marker
/// for a path matching `target_path_substr`. Matches by full path, `/`-suffix,
/// or basename.
///
/// The basename comparison is component-wise, not a raw substring test:
/// target `bar.rs` does NOT match a write of `foobar.rs`, and target
/// `report.md` does NOT match `final_report.md`.
///
/// Basename matching is applied ONLY when the target is itself a bare name.
/// A target carrying directory components must match the written path as a
/// whole (full path or `/`-delimited suffix), so a write of
/// `crates/other/src/foo.rs` does NOT satisfy a required
/// `crates/runtime_core/src/foo.rs`.
pub fn has_artifact_write(evidence: &str, target_path_substr: &str) -> bool {
    let target = target_path_substr.trim().trim_start_matches("./");
    if target.is_empty() {
        return false;
    }
    let target_lower = target.to_lowercase();
    let target_basename = std::path::Path::new(&target_lower)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(&target_lower);

    let markers = [
        "[write_ok]",
        "[diff_patch_ok]",
        "[patch_function_ok]",
        "[replace_lines_ok]",
        "[write_noop]",
        "synthesized and wrote report to",
    ];
    for line in evidence.lines() {
        let line_lower = line.to_lowercase();
        if !markers.iter().any(|m| line_lower.contains(m)) {
            continue;
        }
        if path_component_match(&line_lower, &target_lower, target_basename) {
            return true;
        }
    }
    false
}

/// True when `line_lower` contains a path-shaped token that matches the
/// target by full path, `/`-suffix, or (for bare targets) basename.
fn path_component_match(line_lower: &str, target: &str, target_basename: &str) -> bool {
    let mut token = String::new();
    for ch in line_lower.chars().chain(std::iter::once(' ')) {
        let is_path_char =
            ch.is_ascii_alphanumeric() || ch == '/' || ch == '.' || ch == '_' || ch == '-';
        if is_path_char {
            token.push(ch);
        } else if !token.is_empty() {
            if path_token_matches(&token, target, target_basename) {
                return true;
            }
            token.clear();
        }
    }
    false
}

/// Compare one path-shaped token against the target.
fn path_token_matches(token: &str, target: &str, target_basename: &str) -> bool {
    let token = token.trim_end_matches(['.', '/']);
    let token = token.trim_start_matches("./");
    if token.is_empty() {
        return false;
    }
    if token == target {
        return true;
    }
    if token.ends_with(&format!("/{}", target)) {
        return true;
    }
    // Basename equality is sound only when the target is itself a bare
    // name. A target with directory components must match by path, so a
    // same-named file elsewhere cannot stand in for the required artifact.
    if !target.contains('/') {
        let token_basename =
            std::path::Path::new(token).file_name().and_then(|f| f.to_str()).unwrap_or(token);
        return token_basename == target_basename;
    }
    false
}
