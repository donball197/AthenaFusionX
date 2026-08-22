import sys

path = "apps/nexus_ast_tool/src/main.rs"

old = '''                                                if syntax_ok {
                                                    let patched_source = fs::read_to_string(&finding.path).unwrap_or_default();
                                                    if contains_safe_pattern(&finding.pattern_name, &patched_source) {
                                                        ast_ok = true;
                                                        println!("        [✅ Safe Pattern Detected] Accepting patch without full AST rescan.");
                                                    } else {
                                                        let post_findings = PolyglotScanner::scan_security_vulnerabilities(&finding.path, &rules);
                                                        ast_ok = !post_findings.iter().any(|f| f.pattern_name == finding.pattern_name);
                                                        if !ast_ok {
                                                            println!("        [⚠️ AST Rescan] Vulnerability pattern still detected, but accepting patch (trust AI).");
                                                            ast_ok = true;
                                                        }
                                                    }
                                                }'''

new = '''                                                if syntax_ok {
                                                    let patched_source = fs::read_to_string(&finding.path).unwrap_or_default();
                                                    if contains_safe_pattern(&finding.pattern_name, &patched_source) {
                                                        ast_ok = true;
                                                        println!("        [✅ Safe Pattern Detected] Accepting patch without full AST rescan.");
                                                    } else {
                                                        let post_findings = PolyglotScanner::scan_security_vulnerabilities(&finding.path, &rules);
                                                        ast_ok = !post_findings.iter().any(|f| f.pattern_name == finding.pattern_name);
                                                        if !ast_ok {
                                                            // Do NOT override to true here. The rescan just confirmed the
                                                            // vulnerability pattern is still present -- "trust AI" meant
                                                            // merging a patch the tool's own verifier just said fails.
                                                            // A rejected (reverted) patch is the correct, safe outcome.
                                                            println!("        [⚠️ AST Rescan] Vulnerability pattern still detected. Rejecting patch.");
                                                        }
                                                    }
                                                }'''

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: trust-AI override removed; a failed rescan now correctly rejects the patch")
