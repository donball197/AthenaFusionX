#!/usr/bin/env python3
import sys

path = "apps/nexus_ast_tool/src/main.rs"

with open(path, "r") as f:
    content = f.read()

edits = []

# --- Edit 1: insert resolve_anchor_range + has_real_parameterization helpers ---
old1 = '''    match best.as_slice() {
        [only] => Some((*only, *only + snippet_lines - 1)),
        _ => None, // zero matches or ambiguous — refuse to guess
    }
}
/// Extract the query argument from a call like `mysqli_query($con, $q)`,'''

new1 = '''    match best.as_slice() {
        [only] => Some((*only, *only + snippet_lines - 1)),
        _ => None, // zero matches or ambiguous — refuse to guess
    }
}

/// Generalized version of resolve_anchor for an arbitrary line range.
fn resolve_anchor_range(source: &str, start_line: usize, end_line: usize, expected_text: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    let stored_start = start_line.saturating_sub(1);
    let stored_end = end_line.saturating_sub(1);

    if stored_end < lines.len()
        && lines[stored_start..=stored_end].join("\\n") == expected_text
    {
        return Some((stored_start, stored_end));
    }

    let snippet_lines = expected_text.lines().count().max(1);
    if snippet_lines > lines.len() {
        return None;
    }
    let expected = start_line;
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for i in 0..=(lines.len() - snippet_lines) {
        let end = i + snippet_lines - 1;
        if lines[i..=end].join("\\n") == expected_text {
            candidates.push(((i + 1).abs_diff(expected), i));
        }
    }
    let min_dist = candidates.iter().map(|(d, _)| *d).min()?;
    let best: Vec<usize> = candidates
        .iter()
        .filter(|(d, _)| *d == min_dist)
        .map(|(_, i)| *i)
        .collect();
    match best.as_slice() {
        [only] => Some((*only, *only + snippet_lines - 1)),
        _ => None,
    }
}

/// Does the fully-patched source actually contain real parameterization evidence?
fn has_real_parameterization(source_window: &str) -> bool {
    source_window.contains("mysqli_stmt_bind_param") && source_window.contains("mysqli_stmt_execute")
}

/// Extract the query argument from a call like `mysqli_query($con, $q)`,'''

edits.append(("helpers", old1, new1))

# --- Edit 2: replace the defer-only taint gate with the real multi-site patch attempt ---
old2 = '''                                    if let Some(var_name) = extract_query_var(&finding.sink_text) {
                                        if let Some((c_start, c_end, _)) = nexus_ast_tool::polyglot::find_concat_assignment(
                                            &file_source,
                                            &finding.language.tree_sitter_language(),
                                            &var_name,
                                            finding.sink_start_line,
                                        ) {
                                            taint_deferred_count += 1;
                                            println!(
                                                "        [\\u{26a0}\\u{fe0f} Multi-Site Taint] {} is built via concatenation at line {}-{}, not at the sink. Cannot safely auto-patch in one splice -- needs manual review.",
                                                var_name, c_start, c_end
                                            );
                                            report_md.push_str(&format!(
                                                "### Patch (DEFERRED: multi-site taint -- `{}` built via concatenation at lines {}-{}, not patched)\\n\\n---\\n\\n",
                                                var_name, c_start, c_end
                                            ));
                                            taint_entries.push(format!(
                                                "- `{}` sink lines {}-{} ({}): query variable `{}` built via concatenation at lines {}-{}; auto-heal deferred, requires manual multi-site fix",
                                                file_path_str, finding.sink_start_line, finding.sink_end_line,
                                                finding.pattern_name, var_name, c_start, c_end
                                            ));
                                            continue;
                                        }
                                    }'''

new2 = '''                                    if let (Some(taint_start_line), Some(taint_end_line), Some(taint_text)) =
                                        (finding.taint_start_line, finding.taint_end_line, finding.taint_text.as_deref())
                                    {
                                        println!(
                                            "        [\\u{1f6e0}\\u{fe0f} Auto-Heal (Multi-Site)] Generating secure patch for taint lines {}-{} + sink lines {}-{}...",
                                            taint_start_line, taint_end_line, finding.sink_start_line, finding.sink_end_line
                                        );

                                        let anchors = resolve_anchor(&file_source, &finding)
                                            .zip(resolve_anchor_range(&file_source, taint_start_line, taint_end_line, taint_text));

                                        let Some(((sink_s, sink_e), (taint_s, taint_e))) = anchors else {
                                            println!("        [\\u{274c} Patch Rejected] Anchor drift: multi-site lines no longer match the file.");
                                            failed_fix_count += 1;
                                            report_md.push_str(
                                                "### Patch (REJECTED: anchor drift - multi-site lines no longer match the file)\\n\\n---\\n\\n");
                                            continue;
                                        };

                                        if taint_e >= sink_s {
                                            println!("        [\\u{274c} Patch Rejected] Taint and sink spans overlap; not a valid two-site splice.");
                                            failed_fix_count += 1;
                                            report_md.push_str(
                                                "### Patch (REJECTED: taint/sink spans overlap - refusing to guess)\\n\\n---\\n\\n");
                                            continue;
                                        }

                                        let patch_result = nexus_ast_tool::client::ApiClient::generate_security_patch_multi(
                                            &finding.pattern_name, Some(taint_text), &finding.patch_snippet, &lang_str,
                                        );
                                        let (taint_payload, sink_payload) = match patch_result {
                                            Ok(p) => p,
                                            Err(e) => {
                                                println!("        [\\u{274c} Patch Gen Error] {}", e);
                                                failed_fix_count += 1;
                                                report_md.push_str(&format!(
                                                    "### Patch (FAILED: model error: {})\\n\\n---\\n\\n", e));
                                                continue;
                                            }
                                        };
                                        let Some(taint_payload) = taint_payload else {
                                            println!("        [\\u{274c} Patch Rejected] Model returned no taint-block replacement.");
                                            failed_fix_count += 1;
                                            report_md.push_str("### Patch (REJECTED: empty taint replacement)\\n\\n---\\n\\n");
                                            continue;
                                        };
                                        if sink_payload.trim().is_empty() || taint_payload.trim().is_empty() {
                                            println!("        [\\u{274c} Patch Rejected] Model returned an empty replacement.");
                                            failed_fix_count += 1;
                                            report_md.push_str("### Patch (REJECTED: empty replacement)\\n\\n---\\n\\n");
                                            continue;
                                        }
                                        let cleaned_taint = taint_payload.trim().to_string();
                                        let cleaned_sink = sink_payload.trim().to_string();

                                        let lines: Vec<&str> = file_source.lines().collect();
                                        let crlf = file_source.matches("\\r\\n").count() * 2
                                            > file_source.matches('\\n').count();
                                        let sep = if crlf { "\\r\\n" } else { "\\n" };
                                        let to_sep = |s: &str| if crlf { s.replace('\\n', "\\r\\n") } else { s.to_string() };
                                        let sink_text_sep = to_sep(&cleaned_sink);
                                        let taint_text_sep = to_sep(&cleaned_taint);

                                        let mut new_source = String::with_capacity(file_source.len() + sink_text_sep.len() + taint_text_sep.len());
                                        new_source.push_str(&lines[..taint_s].join(sep));
                                        if taint_s > 0 {
                                            new_source.push_str(sep);
                                        }
                                        new_source.push_str(&taint_text_sep);
                                        new_source.push_str(sep);
                                        new_source.push_str(&lines[taint_e + 1..sink_s].join(sep));
                                        new_source.push_str(sep);
                                        new_source.push_str(&sink_text_sep);
                                        if sink_e + 1 < lines.len() {
                                            new_source.push_str(sep);
                                            new_source.push_str(&lines[sink_e + 1..].join(sep));
                                        }
                                        if file_source.ends_with('\\n') && !new_source.ends_with('\\n') {
                                            new_source.push_str(sep);
                                        }

                                        let ts_ok = finding.language.parses_cleanly(&new_source);
                                        let lint = if ts_ok {
                                            external_lint(&lang_str, &new_source, &finding.path)
                                        } else {
                                            None
                                        };
                                        let syntax_ok = ts_ok && lint.unwrap_or(true);

                                        let param_ok = has_real_parameterization(&new_source);

                                        let ast_ok = if syntax_ok {
                                            let residuals = PolyglotScanner::scan_source(&finding.path, &new_source, &rules);
                                            let taint_delta = cleaned_taint.lines().count() as isize - (taint_e - taint_s + 1) as isize;
                                            let new_sink_start = (sink_s as isize + taint_delta) as usize + 1;
                                            let new_sink_end = new_sink_start + cleaned_sink.lines().count().saturating_sub(1);
                                            !residuals.iter().any(|f| {
                                                f.pattern_name == finding.pattern_name
                                                    && f.sink_start_line + 2 >= new_sink_start
                                                    && f.sink_end_line <= new_sink_end + 2
                                            })
                                        } else {
                                            false
                                        };

                                        if syntax_ok && param_ok && ast_ok {
                                            if let Err(e) = fs::write(&finding.path, &new_source) {
                                                println!("        [\\u{274c} Patch Error] Failed to write verified patch: {}", e);
                                                failed_fix_count += 1;
                                                report_md.push_str(&format!(
                                                    "### Patch (FAILED: I/O write error: {})\\n\\n---\\n\\n", e));
                                                continue;
                                            }
                                            fixed_count += 1;
                                            println!("        [\\u{2705} Patch Verified] Multi-site: syntax OK, real parameterization confirmed, AST rescan confirms sink eliminated.");
                                            report_md.push_str("### Recommended Patch (Auto-Healed, Multi-Site, Verified)\\n");
                                            report_md.push_str(&format!(
                                                "```{}\\n// --- taint site (lines {}-{}) ---\\n{}\\n\\n// --- sink site ---\\n{}\\n```\\n\\n---\\n\\n",
                                                lang_str.to_lowercase(), taint_start_line, taint_end_line, cleaned_taint, cleaned_sink));
                                            continue;
                                        }

                                        let reject_reason = if !ts_ok {
                                            "tree-sitter syntax gate failed"
                                        } else if lint == Some(false) {
                                            "external linter rejected the patch"
                                        } else if !param_ok {
                                            "no real parameterization evidence (mysqli_stmt_bind_param/execute missing) - escape-only or inert patch"
                                        } else {
                                            "AST rescan still detects the sink"
                                        };
                                        failed_fix_count += 1;
                                        println!("        [\\u{274c} Patch Rejected] Multi-site: {}. Nothing was written.", reject_reason);
                                        report_md.push_str(&format!(
                                            "### Recommended Patch (Multi-Site, REJECTED: {} - never written)\\n```{}\\n// taint:\\n{}\\n// sink:\\n{}\\n```\\n\\n---\\n\\n",
                                            reject_reason, lang_str.to_lowercase(), cleaned_taint, cleaned_sink));
                                        continue;
                                    } else if let Some(var_name) = extract_query_var(&finding.sink_text) {
                                        // Fallback safety net: taint_text wasn't captured at
                                        // scan time but the older gate-time lookup still finds a
                                        // live concat assignment. Defer rather than guess.
                                        if let Some((c_start, c_end, _)) = nexus_ast_tool::polyglot::find_concat_assignment(
                                            &file_source,
                                            &finding.language.tree_sitter_language(),
                                            &var_name,
                                            finding.sink_start_line,
                                        ) {
                                            taint_deferred_count += 1;
                                            println!(
                                                "        [\\u{26a0}\\u{fe0f} Multi-Site Taint] {} is built via concatenation at line {}-{}, not at the sink. Cannot safely auto-patch in one splice -- needs manual review.",
                                                var_name, c_start, c_end
                                            );
                                            report_md.push_str(&format!(
                                                "### Patch (DEFERRED: multi-site taint -- `{}` built via concatenation at lines {}-{}, not patched)\\n\\n---\\n\\n",
                                                var_name, c_start, c_end
                                            ));
                                            taint_entries.push(format!(
                                                "- `{}` sink lines {}-{} ({}): query variable `{}` built via concatenation at lines {}-{}; auto-heal deferred, requires manual multi-site fix",
                                                file_path_str, finding.sink_start_line, finding.sink_end_line,
                                                finding.pattern_name, var_name, c_start, c_end
                                            ));
                                            continue;
                                        }
                                    }'''

edits.append(("multi-site branch", old2, new2))

for label, old, new in edits:
    count = content.count(old)
    if count == 0:
        print(f"FAIL ({label}): anchor not found")
        sys.exit(1)
    if count > 1:
        print(f"FAIL ({label}): anchor not unique ({count} matches)")
        sys.exit(1)
    content = content.replace(old, new)

with open(path, "w") as f:
    f.write(content)

print("OK: patched", path)
