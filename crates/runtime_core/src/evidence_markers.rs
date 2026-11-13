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

/// True when `evidence` holds a line-anchored success record for writing `path`:
///   [WRITE_OK] <path>
///   [WRITE_OK] Successfully synthesized and wrote report to <path>
///   [WRITE_NOOP] <path> -- content identical to existing file
/// A no-op means the file already holds exactly the requested content, so the
/// end state the directive asked for is true. It is accepted here only: it is
/// not a mutation (`has_mutation_marker` still rejects it).
pub fn write_path_satisfied(evidence: &str, path: &str) -> bool {
    let target = path.trim();
    let synthesized = format!("Successfully synthesized and wrote report to {}", target);
    evidence.lines().any(|l| {
        if let Some(rest) = l.strip_prefix("[WRITE_OK]") {
            let rest = rest.trim();
            // Accept exact match OR the shared absolute/relative equivalence,
            // mirroring ReadPathAcquired so the two cannot diverge again.
            rest == target
                || rest == synthesized
                || crate::evidence_paths::paths_equivalent(rest, target)
        } else if let Some(rest) = l.strip_prefix("[WRITE_NOOP]") {
            let rest = rest.trim();
            let rest = rest
                .strip_suffix("-- content identical to existing file")
                .map(str::trim_end)
                .unwrap_or(rest);
            rest == target || crate::evidence_paths::paths_equivalent(rest, target)
        } else {
            false
        }
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
/// A criterion prefix is treated as a workspace-relative search root only
/// when it resembles one (absolute path, contains a separator, or names a
/// known top-level root). This keeps `path:term` parsing unambiguous for
/// terms that themselves contain colons, such as Rust module paths.
pub(crate) fn looks_like_workspace_path(s: &str) -> bool {
    // DOT_ROOT: "." / "./" is the workspace root.
    s == "."
        || s == "./"
        || s.starts_with('/')
        || s.contains('/')
        || matches!(s, "crates" | "bins" | "src" | "vendor" | "apps" | "drones")
}

/// True when evidence records a successful, path/query-bound search.
///
/// Accepts either:
///   [SEARCH_CODE_OK] Path: <path> | Query: '<query>'
///   MATCHES:
///
/// or the executor's exact no-match result:
///   [SEARCH_CODE_OK] Path: <path> | Query: '<query>'
///   No code matches found in specified path.
///
/// The no-match phrase must be the exact standalone executor line inside
/// the marker block, and the block must have no MATCHES: section. A phrase
/// embedded in source output is not absence proof. Line-anchored scanning
/// keeps a marker string inside another block's body from being treated
/// as a real marker.
pub fn search_checked_satisfied(evidence: &str, expected: &str) -> bool {
    let expected = expected.trim().to_lowercase();

    let (path_part, term_part) = match expected.split_once(':') {
        Some((p, t)) if looks_like_workspace_path(p.trim()) => {
            // Trim BOTH leading and trailing slashes. The planner may emit
            // `bins/:todo!` (trailing slash from mission text "bins/") or
            // `/bins/:todo!` (absolute path form). Evidence records
            // `Path: .../bins` without a trailing slash. trim_end_matches
            // alone is insufficient for the leading-slash form because
            // `format!("/{}", "/bins")` produces "//bins" which never
            // matches a real evidence path.
            (
                {
                    let t0 = p.trim().trim_matches('/');
                    if t0 == "." {
                        String::new()
                    } else {
                        t0.to_string()
                    }
                },
                t.trim().to_string(),
            )
        }
        _ => (String::new(), expected.clone()),
    };

    if term_part.is_empty() {
        return false;
    }

    let lines: Vec<&str> = evidence.lines().collect();
    let mut i = 0usize;

    while i < lines.len() {
        let Some(header_body) = lines[i].trim_start().strip_prefix("[SEARCH_CODE_OK]") else {
            i += 1;
            continue;
        };

        let header = header_body.trim();
        let mut has_matches = false;
        let mut exact_no_match = false;
        let mut j = i + 1;

        while j < lines.len() {
            let next = lines[j].trim_start();

            if next.starts_with('[')
                && !next.starts_with("[MATCHES")
                && !next.starts_with("[FALLBACK")
            {
                break;
            }

            if next.starts_with("MATCHES:") {
                has_matches = true;
            }

            if next.trim() == "No code matches found in specified path." {
                exact_no_match = true;
            }

            j += 1;
        }

        let successful_outcome = has_matches || exact_no_match;

        if successful_outcome {
            if let Some((path_field, query_field)) = header.split_once("| Query:") {
                if let Some(recorded_path) = path_field.trim().strip_prefix("Path:") {
                    let recorded_path = recorded_path.trim().trim_matches('/').to_lowercase();
                    let recorded_query = query_field
                        .trim()
                        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
                        .to_lowercase();

                    if !recorded_query.is_empty() {
                        let path_ok = path_part.is_empty()
                            || recorded_path == path_part
                            || recorded_path.ends_with(&format!("/{}", path_part));

                        if path_ok && recorded_query.contains(&term_part) {
                            return true;
                        }
                    }
                }
            }
        }

        i = j;
    }

    false
}

pub fn search_term_satisfied(evidence: &str, expected: &str) -> bool {
    let expected = expected.trim().to_lowercase();
    // Split on the FIRST colon only when the prefix looks like a workspace
    // path. Otherwise keep the whole expectation as a term: a term that
    // contains a colon (e.g. "std::io") would otherwise be misparsed as
    // path "std", term "io" and could never match. When path_part is empty,
    // path binding is intentionally skipped below.
    let (path_part, term_part) = match expected.split_once(':') {
        Some((p, t)) if looks_like_workspace_path(p.trim()) => {
            // Trim BOTH leading and trailing slashes. The planner may emit
            // `bins/:todo!` (trailing slash from mission text "bins/") or
            // `/bins/:todo!` (absolute path form). Evidence records
            // `Path: .../bins` without a trailing slash. trim_end_matches
            // alone is insufficient for the leading-slash form because
            // `format!("/{}", "/bins")` produces "//bins" which never
            // matches a real evidence path.
            (
                {
                    let t0 = p.trim().trim_matches('/');
                    if t0 == "." {
                        String::new()
                    } else {
                        t0.to_string()
                    }
                },
                t.trim().to_string(),
            )
        }
        _ => (String::new(), expected.clone()),
    };

    if term_part.is_empty() {
        return false;
    }

    // Parse markers ONLY from lines that BEGIN with [SEARCH_CODE_OK]
    // (after optional leading whitespace). A [SEARCH_CODE_OK] occurring
    // inside a MATCHES: body -- e.g. as a string literal in a test that
    // exercises this parser, or a doc comment documenting the marker
    // format -- is DATA, not a marker. The previous split-on-substring
    // implementation treated any occurrence as a marker start, so any
    // file whose content contained the literal marker string could spoof
    // a search result for a directory that was never searched. Confirmed
    // live: a `crates` search that matched evidence_markers.rs's own
    // unit tests "satisfied" a `bins:...` criterion because the test
    // source contains a fabricated marker string.
    let lines: Vec<&str> = evidence.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let Some(header_body) = lines[i].trim_start().strip_prefix("[SEARCH_CODE_OK]") else {
            i += 1;
            continue;
        };
        let header = header_body.trim();

        // Scan forward for MATCHES:. A new marker line ends the block.
        let mut has_matches = false;
        let mut j = i + 1;
        while j < lines.len() {
            let next = lines[j].trim_start();
            if next.starts_with('[')
                && !next.starts_with("[MATCHES")
                && !next.starts_with("[FALLBACK")
            {
                break;
            }
            if next.starts_with("MATCHES:") {
                has_matches = true;
            }
            j += 1;
        }

        if has_matches {
            if let Some((path_field, query_field)) = header.split_once("| Query:") {
                if let Some(recorded_path) = path_field.trim().strip_prefix("Path:") {
                    let recorded_path = recorded_path.trim().trim_matches('/').to_lowercase();
                    let recorded_query = query_field
                        .trim()
                        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
                        .to_lowercase();
                    if !recorded_query.is_empty() {
                        let path_ok = path_part.is_empty()
                            || recorded_path == path_part
                            || recorded_path.ends_with(&format!("/{}", path_part));
                        if path_ok && recorded_query.contains(&term_part) {
                            return true;
                        }
                    }
                }
            }
        }
        i = j;
    }
    false
}

/// Returns true only when one physical READ_FUNCTION_OK marker record
/// identifies the requested path and function together.
///
/// The path comparison uses the shared absolute/workspace-relative
/// equivalence policy. The function name is an exact match. Evidence from
/// separate records must never be combined to satisfy this predicate.
pub fn read_function_marker_satisfies(
    evidence: &str,
    expected_path: &str,
    expected_function: &str,
) -> bool {
    let expected_path = expected_path.trim();
    let expected_function = expected_function.trim();

    if expected_path.is_empty() || expected_function.is_empty() {
        return false;
    }

    evidence.lines().any(|line| {
        let Some(record) = line.trim_start().strip_prefix("[READ_FUNCTION_OK]") else {
            return false;
        };

        let record = record.trim();
        let Some((record_path, record_function)) = record.rsplit_once(':') else {
            return false;
        };

        crate::evidence_paths::paths_equivalent(record_path.trim(), expected_path)
            && record_function.trim() == expected_function
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

/// Record-scoped, kind-specific patch evidence check.
///
/// Each kind of patch marker has its own wire format emitted by the
/// executor:
///   `[DIFF_PATCH_OK] Applied changes to <path>`
///   `[REPLACE_LINES_OK] Replaced lines N-M in <path>`
///   `[PATCH_FUNCTION_OK] <path>:<fn>` (first line; body carries a
///       human-readable summary referencing the same path/fn)
///   `[PATCH_IDEMPOTENT_OK] <path>:<fn>` (first line; body is
///       "PatchFunction already satisfied; source unchanged.")
///
/// A `PatchPathAcquired { kind: "DIFF_PATCH_OK", .. }` criterion must NOT
/// be satisfied by `[PATCH_FUNCTION_OK]` (or vice versa). Path binding is
/// exact after trim, so an unrelated path that merely *contains* the
/// target is rejected.
pub fn patch_path_satisfied(
    evidence: &str,
    kind: &str,
    path: &str,
    function_name: Option<&str>,
) -> bool {
    fn norm_path(p: &str) -> String {
        let p = p.trim().trim_start_matches("./");
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(rel) = std::path::Path::new(p).strip_prefix(&cwd) {
                return rel.to_string_lossy().to_string();
            }
        }
        p.to_string()
    }
    let target = norm_path(path);
    if target.is_empty() {
        return false;
    }

    match kind {
        "DIFF_PATCH_OK" => evidence.lines().any(|l| {
            if let Some(rest) = l.strip_prefix("[DIFF_PATCH_OK]") {
                let rest = rest.trim();
                let path_part = rest.strip_prefix("Applied changes to ").unwrap_or(rest).trim();
                norm_path(path_part) == target
            } else {
                false
            }
        }),
        "REPLACE_LINES_OK" => evidence.lines().any(|l| {
            if let Some(rest) = l.strip_prefix("[REPLACE_LINES_OK]") {
                let rest = rest.trim();
                match rest.rfind(" in ") {
                    Some(i) => norm_path(&rest[i + 4..]) == target,
                    None => false,
                }
            } else {
                false
            }
        }),
        "PATCH_FUNCTION_OK" => evidence.lines().any(|l| {
            let rest = if let Some(r) = l.strip_prefix("[PATCH_FUNCTION_OK]") {
                r
            } else if let Some(r) = l.strip_prefix("[PATCH_IDEMPOTENT_OK]") {
                r
            } else {
                return false;
            };
            let rest = rest.trim();
            match rest.split_once(':') {
                Some((rec_path, rec_fn)) => {
                    if norm_path(rec_path) != target {
                        return false;
                    }
                    function_name.map_or(true, |f| rec_fn.trim() == f)
                }
                None => false,
            }
        }),
        _ => false,
    }
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
    fn read_function_marker_requires_same_record() {
        assert!(!read_function_marker_satisfies(
            concat!(
                "[READ_FUNCTION_OK] crates/runtime_core/src/foo.rs:actual_a\\n",
                "File: crates/runtime_core/src/foo.rs | Function: actual_a\\n",
                "Observed reference to crates/runtime_core/src/bar.rs\\n",
                "Discussion mentions target_b\\n",
            ),
            "crates/runtime_core/src/bar.rs",
            "target_b",
        ));

        assert!(read_function_marker_satisfies(
            "[READ_FUNCTION_OK] crates/runtime_core/src/bar.rs:target_b\n",
            "crates/runtime_core/src/bar.rs",
            "target_b",
        ));
    }

    #[test]
    fn read_function_marker_uses_path_equivalence() {
        assert!(read_function_marker_satisfies(
            "[READ_FUNCTION_OK] /root/AthenaFusionX/src/foo.rs:target\n",
            "src/foo.rs",
            "target",
        ));

        assert!(!read_function_marker_satisfies(
            "[READ_FUNCTION_OK] vendor/foo.rs:target\\n",
            "foo.rs",
            "target",
        ));
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
    fn patch_path_satisfied_is_kind_specific() {
        let diff_ev = "[DIFF_PATCH_OK] Applied changes to bins/ide_backend\n";
        let fn_ev = "[PATCH_FUNCTION_OK] bins/ide_backend:init\nUpdated function 'init' in bins/ide_backend\n";
        assert!(patch_path_satisfied(diff_ev, "DIFF_PATCH_OK", "bins/ide_backend", None));
        assert!(!patch_path_satisfied(fn_ev, "DIFF_PATCH_OK", "bins/ide_backend", None));
        assert!(patch_path_satisfied(fn_ev, "PATCH_FUNCTION_OK", "bins/ide_backend", Some("init")));
        assert!(!patch_path_satisfied(
            fn_ev,
            "PATCH_FUNCTION_OK",
            "bins/ide_backend",
            Some("other")
        ));
    }

    #[test]
    fn patch_path_satisfied_rejects_superset_path() {
        // An evidence path that merely *contains* the target must not match.
        let ev = "[DIFF_PATCH_OK] Applied changes to bins/ide_backend/src/broker.rs\n";
        assert!(!patch_path_satisfied(ev, "DIFF_PATCH_OK", "bins/ide_backend", None));
        assert!(patch_path_satisfied(ev, "DIFF_PATCH_OK", "bins/ide_backend/src/broker.rs", None));
    }

    #[test]
    fn patch_path_satisfied_replace_lines_format() {
        let ev = "[REPLACE_LINES_OK] Replaced lines 8-10 in src/lib.rs\n";
        assert!(patch_path_satisfied(ev, "REPLACE_LINES_OK", "src/lib.rs", None));
        assert!(!patch_path_satisfied(ev, "REPLACE_LINES_OK", "src/main.rs", None));
    }

    #[test]
    fn search_term_matches_recorded_query_not_body() {
        let ev = "[SEARCH_CODE_OK] Path: src | Query: some_term\nMATCHES:\nline 1\n";
        assert!(search_term_satisfied(ev, "some_term"));
    }

    #[test]
    fn search_term_matches_quoted_query_with_path_scope() {
        let ev =
            "[SEARCH_CODE_OK] Path: vendor/external_repo | Query: 'Cargo.toml'\nMATCHES:\nworkspace member listing\n";
        assert!(search_term_satisfied(ev, "vendor/external_repo:Cargo.toml"));
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
    fn marker_text_inside_matches_body_is_not_a_marker() {
        // A crates search whose MATCHES body contains the literal
        // marker string (as our own unit tests do) must NOT satisfy a
        // bins criterion. This is the exact spoof that let task_3
        // "pass" with only a crates search in the live run.
        let ev = concat!(
            "[SEARCH_CODE_OK] Path: crates | Query: 'unimplemented!'\n",
            "MATCHES:\n",
            "evidence_markers.rs:264:  let bins_ok = \"[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\\nMATCHES:\\nmain.rs:1: unimplemented!()\\n\";\n",
        );
        assert!(
            !search_term_satisfied(ev, "bins:unimplemented!"),
            "a marker string inside a MATCHES body must not spoof a bins search"
        );
        // The real crates marker is still recognised.
        assert!(search_term_satisfied(ev, "crates:unimplemented!"));
    }

    #[test]
    fn bins_criterion_satisfied_only_by_bins_search() {
        let bins_ok = "[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\nMATCHES:\nmain.rs:1: unimplemented!()\n";
        let crates_only = "[SEARCH_CODE_OK] Path: crates | Query: 'unimplemented!'\nMATCHES:\nlib.rs:1: unimplemented!()\n";
        assert!(search_term_satisfied(bins_ok, "bins:unimplemented!"));
        assert!(!search_term_satisfied(crates_only, "bins:unimplemented!"));
    }

    #[test]
    fn bare_term_criterion_accepts_any_path() {
        let ev = "[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\nMATCHES:\nx\n";
        assert!(search_term_satisfied(ev, "unimplemented!"));
    }

    #[test]
    fn no_match_marker_never_satisfies_term_bound_criterion() {
        let ev =
            "[SEARCH_CODE_OK] Path: bins | Query: 'x'\nNo code matches found in specified path.";
        assert!(!search_term_satisfied(ev, "bins:x"));
        assert!(!search_term_satisfied(ev, "x"));
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

    #[test]
    fn artifact_write_rejects_marker_inside_read_body() {
        // Regression: the prior `line_lower.contains(m)` marker gate treated
        // a [READ_OK] whose body contained "[WRITE_OK] target.md" as a real
        // write, falsely satisfying a write contract for a path never written.
        let ev = concat!(
            "[READ_OK] docs/readme.md\n",
            "FULL FILE CONTENT:\n",
            "Example: [WRITE_OK] target.md -- not a real write\n",
        );
        assert!(!has_artifact_write(ev, "target.md"));
        // Sanity: the same marker delivered as a real line-prefixed write
        // is still accepted.
        assert!(has_artifact_write("[WRITE_OK] target.md\n", "target.md"));
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

    // Line-anchored marker gate. The prior check used `line_lower.contains(m)`,
    // which matched marker strings anywhere in a line -- including the body of
    // a [READ_OK] whose file content contained a literal "[WRITE_OK] target.md".
    // Requiring the marker at the start of the trimmed line closes that
    // spoofing vector. The SynthesizeReport emission shape
    // ("[WRITE_OK] Successfully synthesized and wrote report to <path>")
    // still starts with "[WRITE_OK]", so it is unaffected.
    let anchored_markers = [
        "[write_ok]",
        "[diff_patch_ok]",
        "[patch_function_ok]",
        "[patch_idempotent_ok]",
        "[replace_lines_ok]",
        "[write_noop]",
    ];
    for line in evidence.lines() {
        let t = line.trim_start().to_lowercase();
        if !anchored_markers.iter().any(|m| t.starts_with(m)) {
            continue;
        }
        if path_component_match(&t, &target_lower, target_basename) {
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

#[cfg(test)]
mod search_checked_tests {
    use super::{search_checked_satisfied, search_term_satisfied};

    #[test]
    fn accepts_exact_path_bound_no_match() {
        let evidence = concat!(
            "[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\n",
            "No code matches found in specified path.\n"
        );

        assert!(search_checked_satisfied(evidence, "bins:unimplemented!"));
        assert!(!search_term_satisfied(evidence, "bins:unimplemented!"));
    }

    #[test]
    fn accepts_path_bound_positive_matches() {
        let evidence = concat!(
            "[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\n",
            "MATCHES:\n",
            "main.rs:10: unimplemented!()\n"
        );

        assert!(search_checked_satisfied(evidence, "bins:unimplemented!"));
    }

    #[test]
    fn rejects_wrong_path_and_wrong_query() {
        let evidence = concat!(
            "[SEARCH_CODE_OK] Path: crates | Query: 'unimplemented!'\n",
            "No code matches found in specified path.\n"
        );

        assert!(!search_checked_satisfied(evidence, "bins:unimplemented!"));
        assert!(!search_checked_satisfied(evidence, "crates:another_term"));
    }

    #[test]
    fn rejects_unbound_or_inexact_no_match_text() {
        assert!(!search_checked_satisfied("No code matches found in specified path.", "bins:x"));

        assert!(!search_checked_satisfied(
            "[SEARCH_CODE_OK] Path: bins | Query: 'x'\n\
             No code matches found.",
            "bins:x"
        ));
    }

    #[test]
    fn rejects_bare_marker_with_no_outcome() {
        let evidence = "[SEARCH_CODE_OK] Path: bins | Query: 'x'\n";
        assert!(!search_checked_satisfied(evidence, "bins:x"));
    }

    #[test]
    fn no_match_phrase_in_matches_body_is_not_absence_proof() {
        let evidence = concat!(
            "[SEARCH_CODE_OK] Path: bins | Query: 'x'\n",
            "MATCHES:\n",
            "main.rs:1: // No code matches found in specified path.\n"
        );

        // Accepted because MATCHES exists (a real hit was found),
        // not because the trailing phrase proves absence.
        assert!(search_checked_satisfied(evidence, "bins:x"));
    }
}

#[cfg(test)]
mod trailing_slash_regression_tests {
    use super::*;

    #[test]
    fn all_slash_forms_satisfy() {
        let ev = concat!(
            "[SEARCH_CODE_OK] Path: /root/AthenaFusionX/bins | Query: 'todo!'\n",
            "No code matches found in specified path.\n"
        );
        // All three criteria spellings must resolve to the same match:
        assert!(search_checked_satisfied(ev, "bins:todo!"), "bare form");
        assert!(
            search_checked_satisfied(ev, "bins/:todo!"),
            "trailing-slash form (planner copies from 'bins/' mission text)"
        );
        assert!(
            search_checked_satisfied(ev, "/bins/:todo!"),
            "leading+trailing slash form (absolute path intent)"
        );
    }

    #[test]
    fn positive_matches_also_normalize() {
        let ev = concat!(
            "[SEARCH_CODE_OK] Path: /root/AthenaFusionX/crates | Query: 'todo!'\n",
            "MATCHES:\n",
            "crates/foo.rs:1: todo!()\n"
        );
        assert!(search_checked_satisfied(ev, "crates/:todo!"));
        assert!(search_checked_satisfied(ev, "crates:todo!"));
        assert!(search_checked_satisfied(ev, "/crates/:todo!"));
    }

    #[test]
    fn wrong_path_still_rejected() {
        let ev = concat!(
            "[SEARCH_CODE_OK] Path: /root/AthenaFusionX/bins | Query: 'todo!'\n",
            "No code matches found in specified path.\n"
        );
        assert!(!search_checked_satisfied(ev, "apps/:todo!"));
        assert!(!search_checked_satisfied(ev, "crates/:todo!"));
        assert!(!search_checked_satisfied(ev, "root/:todo!"));
    }
}
