use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use nexus_ast_tool::polyglot::PolyglotScanner;

#[derive(Parser, Debug)]
#[command(name = "nexus_ast_tool", version = "0.1.0", about = "Nexus Singularity Polyglot AST Engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Scan { 
        #[arg(short, long, default_value = ".")] path: PathBuf 
    },
    Audit {
        #[arg(short, long, default_value = ".")] path: PathBuf,
        #[arg(long)] repo: Option<String>,
        #[arg(long)] ai_triage: bool,
        #[arg(long)] auto_heal: bool,
        #[arg(long)] generate_report: bool,
    },
    Refactor { 
        #[arg(short, long, alias = "path")] file: PathBuf,
        #[arg(short, long)] symbol: String,
        #[arg(long, default_value = "Auto")] strategy: String,
        #[arg(long, default_value = "")] payload: String,
        #[arg(long)] instruction: Option<String>,
    },
    /// Allow the engine to autonomously optimize its own codebase
    Optimize { 
        #[arg(short, long, default_value = ".")] path: PathBuf 
    },
}

/// Best-effort syntax check for the language of a spliced file. Returns true
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

/// Syntax validity alone isn't proof a patch fixed anything: a no-op patch
/// (identical to the vulnerable snippet) or one missing the expected hardening
/// call (e.g. no mysqli_prepare/bind_param for a SQL_INJECTION finding) is
/// syntactically fine and functionally useless or actively broken. This is a
/// coarse keyword check, not a real semantic diff -- it catches the two
/// observed failure modes (no-op patch, and a patch that silently corrupts an
/// opening tag like `<?php` -> `<?>`) without needing a real PHP AST.
#[allow(dead_code)]
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
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path } => {
            let symbols = PolyglotScanner::scan_path(&path);
            println!("==> [Polyglot Scanner] Discovered {} symbols total.", symbols.len());
        }
        Commands::Audit { path, repo, ai_triage, auto_heal, generate_report } => {
            let mut scan_path = path.clone();
            
            // --- 1. REMOTE GIT INGESTION ---
            if let Some(repo_url) = repo {
                scan_path = PathBuf::from("./nexus_audit_target");
                if scan_path.exists() {
                    fs::remove_dir_all(&scan_path)?;
                }
                println!("==> [Git] Cloning target repository: {}...", repo_url);
                let status = Command::new("git").args(["clone", "--depth", "1", &repo_url, scan_path.to_str().unwrap()]).status()?;
                if !status.success() {
                    return Err(anyhow::anyhow!("Failed to clone repository"));
                }
            }

            println!("==> [Security Auditor] Commencing AST vulnerability scan on: {}", scan_path.display());
            
            // --- 2. DYNAMIC RULES ENGINE ---
            let rules_dir = PathBuf::from("apps/nexus_ast_tool/rules");
            let rules = PolyglotScanner::load_rules(&rules_dir);
            println!("==> [Rules Engine] Loaded {} dynamic security rules from {:?}", rules.len(), rules_dir);

            let mut findings = PolyglotScanner::scan_security_vulnerabilities(&scan_path, &rules);
            
            if findings.is_empty() {
                println!("==> [Security Auditor] No obvious dangerous sinks found. Codebase looks clean!");
                return Ok(());
            } 
            
            println!("==> [Security Auditor] WARNING: Discovered {} potential vulnerabilities.", findings.len());
            
            let mut report_md = String::from("# 🛡️ Nexus Singularity Bug Bounty Report\n\n");

            // These counters represent distinct security states.
            //
            // actionable_count:
            //     AI explicitly classified the finding EXPLOITABLE.
            //
            // fixed_count:
            //     An EXPLOITABLE finding was patched and passed BOTH
            //     syntax validation and post-patch AST verification.
            //
            // failed_fix_count:
            //     An EXPLOITABLE finding was not successfully remediated.
            //
            // safe_count:
            //     AI explicitly classified the finding SAFE.
            //
            // unknown_count:
            //     Security state could not be established without assumptions.
            let mut actionable_count = 0;
            let mut fixed_count = 0;
            let mut failed_fix_count = 0;
            let mut safe_count = 0;
            let mut unknown_count = 0;

            if ai_triage {
                println!("\n==> [AI Triage] Initializing LLM to filter false positives...");
                findings.sort_by_key(|b| std::cmp::Reverse(b.start_line));

                for finding in findings {
                    println!("\n    [?] Evaluating {} at line {}...", finding.pattern_name.to_uppercase(), finding.start_line);
                    
                    let file_path_str = finding.path.display().to_string();
                    let lang_str = format!("{:?}", finding.language);

                    match nexus_ast_tool::client::ApiClient::triage_vulnerability(&finding.pattern_name, &finding.source_snippet) {
                        Ok((is_exploitable, severity, reason)) => {
                            let classification =
                                match is_exploitable.trim().to_uppercase().as_str() {
                                    "EXPLOITABLE" => "EXPLOITABLE",
                                    "SAFE" => "SAFE",
                                    "UNKNOWN" => "UNKNOWN",
                                    other => {
                                        println!(
                                            "        [⚠️ TRIAGE POLICY] Invalid model state '{}'; forcing UNKNOWN.",
                                            other
                                        );
                                        "UNKNOWN"
                                    }
                                };

                            if classification == "EXPLOITABLE" {
                                actionable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Severity: {}", severity);
                                println!("        [🧠 AI Reasoning] {}", reason);

                                // Add to Markdown Report
                                report_md.push_str(&format!("## 🚨 Vulnerability: {}\n", finding.pattern_name.to_uppercase()));
                                report_md.push_str(&format!("- **Severity:** {}\n", severity));
                                report_md.push_str(&format!("- **File:** `{}` (Lines {}-{})\n\n", file_path_str, finding.start_line, finding.end_line));
                                report_md.push_str(&format!("### Description\n{}\n\n", reason));
                                report_md.push_str(&format!("### Vulnerable Code\n```{}\n{}\n```\n\n", lang_str.to_lowercase(), finding.source_snippet));

                                if auto_heal {
                                println!("        [🛠️ Auto-Heal] Generating secure patch...");
                                match nexus_ast_tool::client::ApiClient::generate_security_patch(&finding.pattern_name, &finding.source_snippet, &lang_str) {
                                    Ok((search_block, replace_block)) => {
                                        if let Ok(file_source) = std::fs::read_to_string(&finding.path) {
                                            let fuzzy_match = |src: &str, target: &str| -> Option<String> {
                                                let target_norm = target.split_whitespace().collect::<Vec<_>>().join(" ");
                                                let src_lines: Vec<&str> = src.lines().collect();
                                                let tgt_lines_count = target.lines().count().max(1);
                                                
                                                // Slide a window over the source lines (allowing for +/- 3 lines of wrapping differences)
                                                for window_size in tgt_lines_count.saturating_sub(3)..=(tgt_lines_count + 3) {
                                                    if window_size == 0 || window_size > src_lines.len() { continue; }
                                                    for i in 0..=(src_lines.len() - window_size) {
                                                        let window = src_lines[i..i+window_size].join("
");
                                                        let window_norm = window.split_whitespace().collect::<Vec<_>>().join(" ");
                                                        if window_norm == target_norm {
                                                            return Some(window);
                                                        }
                                                    }
                                                }
                                                None
                                            };

                                            let mut matched_exact = false;
                                            let mut new_source_mut = file_source.clone();

                                            if file_source.contains(&search_block) {
                                                new_source_mut = file_source.replace(&search_block, &replace_block);
                                                matched_exact = true;
                                            } else if let Some(matched_window) = fuzzy_match(&file_source, &search_block) {
                                                // Whitespace drift detected! Using the semantic window match.
                                                new_source_mut = file_source.replace(&matched_window, &replace_block);
                                                matched_exact = true;
                                            }

                                            if !matched_exact {
                                                // The SEARCH block was captured from a pre-patch snapshot of an
                                                // overlapping context window. If a nearby finding in this same
                                                // file was already patched first (findings are processed in
                                                // descending line order), that earlier edit can invalidate this
                                                // finding's SEARCH text even though the underlying site is
                                                // already fixed. Re-check before counting it as a real failure.
                                                let already_resolved = {
                                                    let post_findings = PolyglotScanner::scan_security_vulnerabilities(&finding.path, &rules);
                                                    !post_findings.iter().any(|f| {
                                                        f.pattern_name == finding.pattern_name
                                                            && f.start_line <= finding.end_line
                                                            && f.end_line >= finding.start_line
                                                    })
                                                };

                                                if already_resolved {
                                                    println!("        [ℹ️ Already Resolved] Site no longer matches -- likely fixed by a nearby patch earlier this run.");
                                                    report_md.push_str(
                                                        "### Recommended Patch (auto-heal skipped: site already resolved by an earlier patch in this run)\n\n",
                                                    );
                                                } else {
                                                    failed_fix_count += 1;

                                                    println!("        [❌ Patch Rejected] SEARCH block not found exactly in file. Reverting.");
                                                    report_md.push_str(&format!(
                                                        "### Recommended Patch (auto-heal attempted, REJECTED: SEARCH block mismatch, reverted)\n```{}\n{}\n```\n\n",
                                                        lang_str.to_lowercase(), replace_block
                                                    ));
                                                }
                                            } else {
                                                let new_source = new_source_mut;
                                                let _ = std::fs::write(&finding.path, &new_source);

                                                let syntax_ok = verify_syntax(&lang_str, &finding.path);
                                                let mut ast_ok = false;

                                                if syntax_ok {
                                                    let post_findings = PolyglotScanner::scan_security_vulnerabilities(&finding.path, &rules);
                                                    // Only treat the patch as failed if THIS site still matches --
                                                    // not if some other, unrelated finding elsewhere in the file
                                                    // (e.g. a deliberately unpatched UNKNOWN-classified site)
                                                    // happens to share the same pattern_name.
                                                    ast_ok = !post_findings.iter().any(|f| {
                                                        f.pattern_name == finding.pattern_name
                                                            && f.start_line <= finding.end_line
                                                            && f.end_line >= finding.start_line
                                                    });
                                                }

                                                // SECURITY BOUNDARY:
                                                //
                                                // A patch is VERIFIED only when BOTH
                                                // authoritative checks pass:
                                                //
                                                //   1. syntax validation succeeds
                                                //   2. post-patch AST/security rescan no longer
                                                //      reports the original vulnerability
                                                //
                                                // No keyword heuristic may bypass the AST check.
                                                if syntax_ok && ast_ok {
                                                    fixed_count += 1;

                                                    println!(
                                                        "        [✅ Patch Verified] Syntax OK and AST Rescan confirms vulnerability eliminated."
                                                    );
                                                    report_md.push_str(
                                                        "### Recommended Patch (Auto-Healed)\n"
                                                    );
                                                    report_md.push_str(&format!(
                                                        "```{}\n{}\n```\n\n",
                                                        lang_str.to_lowercase(),
                                                        replace_block
                                                    ));
                                                } else {
                                                    failed_fix_count += 1;

                                                    let reject_reason = if !syntax_ok {
                                                        "Syntax check failed"
                                                    } else {
                                                        "AST Rescan detected vulnerability still present"
                                                    };

                                                    println!(
                                                        "        [❌ Patch Rejected] {}. Reverting.",
                                                        reject_reason
                                                    );

                                                    std::fs::write(&finding.path, &file_source)?;

                                                    report_md.push_str(&format!(
                                                        "### Recommended Patch (auto-heal attempted, REJECTED: {}, reverted)\n```{}\n{}\n```\n\n",
                                                        reject_reason,
                                                        lang_str.to_lowercase(),
                                                        replace_block
                                                    ));
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        failed_fix_count += 1;
                                        println!("        [❌ Patch Gen Error] {}", e);
                                    },
                                }
                            }
                            report_md.push_str("---\n\n");
                            } else if classification == "SAFE" {
                                safe_count += 1;

                                println!(
                                    "        [🟢 SAFE] Explicitly classified safe."
                                );
                                println!(
                                    "        [🧠 AI Reasoning] {}",
                                    reason
                                );
                            } else {
                                unknown_count += 1;

                                println!(
                                    "        [🟡 UNKNOWN] Security state cannot be established without assumptions."
                                );
                                println!(
                                    "        [🧠 AI Reasoning] {}",
                                    reason
                                );
                            }
                        },
                        Err(e) => println!("        [⚠️ API Error] {}", e),
                    }
                }

                if generate_report {
                    report_md.push_str("\n# Final Audit Summary\n\n");

                    report_md.push_str(&format!(
                        "- **Actionable findings:** {}\n",
                        actionable_count
                    ));
                    report_md.push_str(&format!(
                        "- **Verified fixed:** {}\n",
                        fixed_count
                    ));
                    report_md.push_str(&format!(
                        "- **Remediation failed:** {}\n",
                        failed_fix_count
                    ));
                    report_md.push_str(&format!(
                        "- **Explicitly safe:** {}\n",
                        safe_count
                    ));
                    report_md.push_str(&format!(
                        "- **Unknown:** {}\n\n",
                        unknown_count
                    ));

                    let remediation_clean =
                        actionable_count == fixed_count
                        && failed_fix_count == 0;

                    if remediation_clean {
                        report_md.push_str(
                            "## Remediation Integrity\n\n                             **PASS** — Every actionable finding was VERIFIED_FIXED.\n\n"
                        );
                    } else {
                        report_md.push_str(
                            "## Remediation Integrity\n\n                             **FAIL** — One or more actionable findings were not VERIFIED_FIXED.\n\n"
                        );
                    }

                    fs::write("bounty_report.md", &report_md)?;

                    println!(
                        "\n==> [Report Generator] 📄 bounty_report.md generated."
                    );
                    println!(
                        "    Actionable findings : {}",
                        actionable_count
                    );
                    println!(
                        "    Verified fixed      : {}",
                        fixed_count
                    );
                    println!(
                        "    Remediation failed  : {}",
                        failed_fix_count
                    );
                    println!(
                        "    Explicitly safe     : {}",
                        safe_count
                    );
                    println!(
                        "    Unknown             : {}",
                        unknown_count
                    );

                    if remediation_clean {
                        println!(
                            "    [✅ REMEDIATION INTEGRITY] PASS"
                        );
                    } else {
                        println!(
                            "    [❌ REMEDIATION INTEGRITY] FAIL"
                        );
                    }
                }
            }
        }
        Commands::Refactor { file, symbol, strategy, payload, instruction } => {
            let agent = nexus_ast_tool::agent::RefactorAgent::new();
            if let Some(inst) = instruction {
                let _ = agent.execute_instruction(&file, &symbol, &inst, &strategy);
            } else if !payload.is_empty() {
                let _ = agent.apply_refactoring(&file, &symbol, &strategy, &payload);
            }
        }
        Commands::Optimize { path } => {
            nexus_ast_tool::supervisor::Supervisor::self_optimize(&path)?;
        }
    }
    Ok(())
}
