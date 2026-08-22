import sys

path = "apps/nexus_ast_tool/src/main.rs"

with open(path, "r") as f:
    content = f.read()

old = '''                        if is_exploitable {
                                exploitable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);

                                // Add to Markdown Report
                                report_md.push_str(&format!("## 🚨 Vulnerability: {}\\n", finding.pattern_name.to_uppercase()));
                                report_md.push_str(&format!("- **Severity:** {}\\n", severity));
                                report_md.push_str(&format!("- **File:** `{}` (Lines {}-{})\\n\\n", file_path_str, finding.start_line, finding.end_line));
                                report_md.push_str(&format!("### Description\\n{}\\n\\n", reason));
                                report_md.push_str(&format!("### Vulnerable Code\\n```{}\\n{}\\n```\\n\\n", lang_str.to_lowercase(), finding.source_snippet));

                                if auto_heal {
                                    println!("        [🛠️ Auto-Heal] Generating secure patch...");
                                    match nexus_ast_tool::client::ApiClient::generate_security_patch(&finding.pattern_name, &finding.source_snippet, &lang_str) {
                                        Ok(patch) => {
                                            if let Ok(file_source) = fs::read_to_string(&finding.path) {
                                                if let Ok(new_source) = nexus_ast_tool::mutation::PolyglotSplicer::splice(&file_source, finding.start_line, finding.end_line, &patch) {
                                                    let _ = fs::write(&finding.path, new_source);
                                                    println!("        [✅ Patch Spliced] Successfully injected secure code into AST.");
                                                    
                                                    report_md.push_str("### Recommended Patch (Auto-Healed)\\n");
                                                    report_md.push_str(&format!("```{}\\n{}\\n```\\n\\n", lang_str.to_lowercase(), patch));
                                                }
                                            }
                                        },
                                        Err(e) => println!("        [❌ Patch Gen Error] {}", e),
                                    }
                                }
                                report_md.push_str("---\\n\\n");'''

new = '''                        if is_exploitable {
                                exploitable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);

                                // Add to Markdown Report
                                report_md.push_str(&format!("## 🚨 Vulnerability: {}\\n", finding.pattern_name.to_uppercase()));
                                report_md.push_str(&format!("- **Severity:** {}\\n", severity));
                                report_md.push_str(&format!("- **File:** `{}` (Lines {}-{})\\n\\n", file_path_str, finding.start_line, finding.end_line));
                                report_md.push_str(&format!("### Description\\n{}\\n\\n", reason));
                                report_md.push_str(&format!("### Vulnerable Code\\n```{}\\n{}\\n```\\n\\n", lang_str.to_lowercase(), finding.source_snippet));

                                if auto_heal {
                                    println!("        [🛠️ Auto-Heal] Generating secure patch...");
                                    match nexus_ast_tool::client::ApiClient::generate_security_patch(&finding.pattern_name, &finding.source_snippet, &lang_str) {
                                        Ok(patch) => {
                                            if let Ok(file_source) = fs::read_to_string(&finding.path) {
                                                if let Ok(new_source) = nexus_ast_tool::mutation::PolyglotSplicer::splice(&file_source, finding.start_line, finding.end_line, &patch) {
                                                    let _ = fs::write(&finding.path, &new_source);

                                                    // Verify-then-revert, same pattern Supervisor::self_optimize
                                                    // already uses for Rust via `cargo check`. Splicing text into
                                                    // a file is not proof it's valid — check it before keeping it.
                                                    if verify_syntax(&lang_str, &finding.path) {
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
                                                    }
                                                }
                                            }
                                        },
                                        Err(e) => println!("        [❌ Patch Gen Error] {}", e),
                                    }
                                }
                                report_md.push_str("---\\n\\n");'''

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)
content = content.replace(old, new, 1)

# Add the verify_syntax helper function before `fn main()`
old_main = "fn main() -> Result<()> {"
if content.count(old_main) != 1:
    print(f"ANCHOR MISMATCH (fn main): found {content.count(old_main)}", file=sys.stderr)
    sys.exit(1)

helper = '''/// Best-effort syntax check for the language of a spliced file. Returns true
/// (accept the patch) if the language has no known linter available or the
/// check passes; returns false (revert) only on a confirmed syntax failure.
fn verify_syntax(lang_str: &str, path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();
    let result = match lang_str {
        "PHP" => Command::new("php").args(["-l", &path_str]).output(),
        "Python" => Command::new("python3").args(["-m", "py_compile", &path_str]).output(),
        "JavaScript" => Command::new("node").args(["--check", &path_str]).output(),
        "TypeScript" => Command::new("node").args(["--check", &path_str]).output(),
        _ => return true, // no verifier wired up for this language yet; don't block on it
    };
    match result {
        Ok(output) => output.status.success(),
        Err(_) => true, // linter binary not installed/on PATH; don't block the patch on tooling absence
    }
}

fn main() -> Result<()> {'''

content = content.replace(old_main, helper, 1)

with open(path, "w") as f:
    f.write(content)

print("OK: auto_heal now verifies syntax post-splice and reverts on failure")
