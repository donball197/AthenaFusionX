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
            let mut exploitable_count = 0;

            if ai_triage {
                println!("\n==> [AI Triage] Initializing LLM to filter false positives...");
                findings.sort_by_key(|b| std::cmp::Reverse(b.start_line));

                for finding in findings {
                    println!("\n    [?] Evaluating {} at line {}...", finding.pattern_name.to_uppercase(), finding.start_line);
                    
                    let file_path_str = finding.path.display().to_string();
                    let lang_str = format!("{:?}", finding.language);

                    match nexus_ast_tool::client::ApiClient::triage_vulnerability(&finding.pattern_name, &finding.source_snippet) {
                        Ok((is_exploitable, severity, reason)) => {
                            if is_exploitable {
                                exploitable_count += 1;
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
                                        Ok(patch) => {
                                            if let Ok(file_source) = fs::read_to_string(&finding.path) {
                                                if let Ok(new_source) = nexus_ast_tool::mutation::PolyglotSplicer::splice(&file_source, finding.start_line, finding.end_line, &patch) {
                                                    let _ = fs::write(&finding.path, &new_source);

                                                    // Verify-then-revert, same pattern Supervisor::self_optimize
                                                    // already uses for Rust via `cargo check`. Splicing text into
                                                    // a file is not proof it's valid — check it before keeping it.
                                                    let syntax_ok = verify_syntax(&lang_str, &finding.path);
                                                    let fix_ok = looks_like_real_fix(&finding.pattern_name, &finding.source_snippet, &patch);

                                                    if syntax_ok && fix_ok {
                                                        println!("        [✅ Patch Spliced] Verified and injected secure code into AST.");
                                                        report_md.push_str("### Recommended Patch (Auto-Healed)\n");
                                                        report_md.push_str(&format!("```{}\n{}\n```\n\n", lang_str.to_lowercase(), patch));
                                                    } else {
                                                        let reason = if !syntax_ok {
                                                            "syntax check failed"
                                                        } else {
                                                            "patch is a no-op or missing the expected fix (e.g. no prepare/bind_param)"
                                                        };
                                                        println!("        [❌ Patch Rejected] {}. Reverting.", reason);
                                                        let _ = fs::write(&finding.path, &file_source);
                                                        report_md.push_str(&format!(
                                                            "### Recommended Patch (auto-heal attempted, REJECTED: {}, reverted)\n```{}\n{}\n```\n\n",
                                                            reason, lang_str.to_lowercase(), patch
                                                        ));
                                                    }
                                                }
                                            }
                                        },
                                        Err(e) => println!("        [❌ Patch Gen Error] {}", e),
                                    }
                                }
                                report_md.push_str("---\n\n");
                            } else {
                                println!("        [🟢 FALSE POSITIVE] Safe.\n        [🧠 AI Reasoning] {}", reason);
                            }
                        },
                        Err(e) => println!("        [⚠️ API Error] {}", e),
                    }
                }

                if generate_report && exploitable_count > 0 {
                    let _ = fs::write("bounty_report.md", &report_md);
                    println!("\n==> [Report Generator] 📄 Successfully generated 'bounty_report.md' with {} actionable bugs!", exploitable_count);
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
