import sys

path = "apps/nexus_ast_tool/src/main.rs"

with open(path, "r") as f:
    content = f.read()

# --- 1. Add looks_like_real_fix() next to verify_syntax() ---
old_helper_anchor = '''    match result {
        Ok(output) => output.status.success(),
        Err(_) => true, // linter binary not installed/on PATH; don't block the patch on tooling absence
    }
}'''

new_helper = old_helper_anchor + '''

/// Syntax validity alone isn't proof a patch fixed anything: a no-op patch
/// (identical to the vulnerable snippet) or one missing the expected hardening
/// call (e.g. no mysqli_prepare/bind_param for a SQL_INJECTION finding) is
/// syntactically fine and functionally useless or actively broken. This is a
/// coarse keyword check, not a real semantic diff -- it catches the two
/// observed failure modes (no-op patch, and a patch that silently corrupts an
/// opening tag like `<?php` -> `<?>`) without needing a real PHP AST.
fn looks_like_real_fix(pattern_name: &str, original_snippet: &str, patch: &str) -> bool {
    if patch.trim() == original_snippet.trim() {
        return false; // no-op: nothing was actually changed
    }
    if patch.contains("<?>") {
        return false; // observed failure: LLM truncated `<?php` into an inert tag
    }
    let patch_lower = patch.to_lowercase();
    match pattern_name {
        "SQL_INJECTION" => {
            patch_lower.contains("prepare") || patch_lower.contains("bind_param")
        }
        "OS_COMMAND" | "PYTHON_CODE_EXECUTION" | "PHP_COMMAND_INJECTION" => {
            !patch_lower.contains("eval(") && !patch_lower.contains("exec(")
                && !patch_lower.contains("system(") && !patch_lower.contains("shell_exec(")
                && !patch_lower.contains("popen(") && !patch_lower.contains("passthru(")
        }
        "JS_XSS" => !patch_lower.contains("dangerouslysetinnerhtml") && !patch_lower.contains("eval("),
        _ => true, // no specific marker known for this rule id; syntax check is the only gate
    }
}'''

if content.count(old_helper_anchor) != 1:
    print(f"ANCHOR MISMATCH (verify_syntax tail): found {content.count(old_helper_anchor)}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old_helper_anchor, new_helper, 1)

# --- 2. Combine the semantic check into the accept/reject branch ---
old_branch = '''                                                    if verify_syntax(&lang_str, &finding.path) {
                                                        println!("        [✅ Patch Spliced] Verified and injected secure code into AST.");
                                                        report_md.push_str("### Recommended Patch (Auto-Healed)\\n");
                                                        report_md.push_str(&format!("```{}\\n{}\\n```\\n\\n", lang_str.to_lowercase(), patch));
                                                    } else {
                                                        println!("        [❌ Patch Rejected] Syntax check failed post-splice. Reverting.");
                                                        let _ = fs::write(&finding.path, &file_source);
                                                        report_md.push_str(&format!(
                                                            "### Recommended Patch (auto-heal attempted, FAILED syntax check, reverted)\\n```{}\\n{}\\n```\\n\\n",
                                                            lang_str.to_lowercase(), patch
                                                        ));
                                                    }'''

new_branch = '''                                                    let syntax_ok = verify_syntax(&lang_str, &finding.path);
                                                    let fix_ok = looks_like_real_fix(&finding.pattern_name, &finding.source_snippet, &patch);

                                                    if syntax_ok && fix_ok {
                                                        println!("        [✅ Patch Spliced] Verified and injected secure code into AST.");
                                                        report_md.push_str("### Recommended Patch (Auto-Healed)\\n");
                                                        report_md.push_str(&format!("```{}\\n{}\\n```\\n\\n", lang_str.to_lowercase(), patch));
                                                    } else {
                                                        let reason = if !syntax_ok {
                                                            "syntax check failed"
                                                        } else {
                                                            "patch is a no-op or missing the expected fix (e.g. no prepare/bind_param)"
                                                        };
                                                        println!("        [❌ Patch Rejected] {}. Reverting.", reason);
                                                        let _ = fs::write(&finding.path, &file_source);
                                                        report_md.push_str(&format!(
                                                            "### Recommended Patch (auto-heal attempted, REJECTED: {}, reverted)\\n```{}\\n{}\\n```\\n\\n",
                                                            reason, lang_str.to_lowercase(), patch
                                                        ));
                                                    }'''

if content.count(old_branch) != 1:
    print(f"ANCHOR MISMATCH (accept/reject branch): found {content.count(old_branch)}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old_branch, new_branch, 1)

with open(path, "w") as f:
    f.write(content)

print("OK: semantic no-op/corruption check added on top of syntax check")
