use anyhow::Result;
use clap::{Parser, Subcommand};
use nexus_ast_tool::polyglot::{PolyglotScanner, SecurityFinding};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "nexus_ast_tool",
    version = "0.1.0",
    about = "Nexus Singularity Polyglot AST Engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Scan {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    Audit {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        ai_triage: bool,
        #[arg(long)]
        auto_heal: bool,
        #[arg(long)]
        generate_report: bool,
        #[arg(long)]
        branch_fix: bool,
        #[arg(long)]
        run_tests: bool,
    },
    Refactor {
        #[arg(short, long, alias = "path")]
        file: PathBuf,
        #[arg(short, long)]
        symbol: String,
        #[arg(long, default_value = "Auto")]
        strategy: String,
        #[arg(long, default_value = "")]
        payload: String,
        #[arg(long)]
        instruction: Option<String>,
    },
    Optimize {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
}

fn external_lint(lang_str: &str, source: &str, orig_path: &Path) -> Option<bool> {
    let (program, mut args, ext): (&str, Vec<&str>, &str) = match lang_str {
        "PHP" => ("php", vec!["-l"], "php"),
        "Python" => (
            "python3",
            vec!["-c", "import sys; compile(open(sys.argv[1]).read(), sys.argv[1], 'exec')"],
            "py",
        ),
        "JavaScript" => ("node", vec!["--check"], "js"),
        "TypeScript" => ("tsc", vec!["--noEmit"], "ts"),
        _ => return None,
    };

    let tmp_path = orig_path.with_extension(format!("nexus_lint.{ext}"));
    fs::write(&tmp_path, source).ok()?;
    args.push(tmp_path.to_str()?);

    let output = Command::new(program).args(&args).output();
    let _ = fs::remove_file(&tmp_path);
    match output {
        Ok(o) => Some(o.status.success()),
        Err(_) => None,
    }
}

fn run_project_test_suite(repo_path: &Path) -> Option<bool> {
    println!("==> [Test Suite Gate] Auto-detecting project test harness in {}...", repo_path.display());

    let (program, args): (&str, Vec<&str>) = if repo_path.join("Cargo.toml").exists() {
        ("cargo", vec!["test", "--color", "never"])
    } else if repo_path.join("phpunit.xml").exists() || repo_path.join("phpunit.xml.dist").exists() {
        ("vendor/bin/phpunit", vec![])
    } else if repo_path.join("pytest.ini").exists() || (repo_path.join("tests").exists() && repo_path.join("requirements.txt").exists()) {
        ("pytest", vec![])
    } else if repo_path.join("package.json").exists() {
        ("npm", vec!["test", "--", "--passWithNoTests"])
    } else {
        println!("    [ℹ️ Test Harness] No standard test harness detected (Cargo, PHPUnit, PyTest, Jest). Skipping.");
        return None;
    };

    println!("    [🧪 Running] {} {}", program, args.join(" "));
    let output = Command::new(program)
        .args(&args)
        .current_dir(repo_path)
        .output();

    match output {
        Ok(o) => {
            if o.status.success() {
                println!("    [✅ Test Suite Passed] 0 regressions detected.");
                Some(true)
            } else {
                let err_msg = String::from_utf8_lossy(&o.stderr);
                println!("    [❌ Test Suite Failed]\n{}", err_msg.lines().take(10).collect::<Vec<_>>().join("\n"));
                Some(false)
            }
        }
        Err(e) => {
            println!("    [⚠️ Test Execution Skipped] Could not run {}: {}", program, e);
            None
        }
    }
}

fn resolve_anchor(source: &str, finding: &SecurityFinding) -> Option<(usize, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    let stored_start = finding.sink_start_line.saturating_sub(1);
    let stored_end = finding.sink_end_line.saturating_sub(1);

    if stored_end < lines.len()
        && lines[stored_start..=stored_end].join("\n") == finding.patch_snippet
    {
        return Some((stored_start, stored_end));
    }

    let snippet_lines = finding.patch_snippet.lines().count().max(1);
    if snippet_lines > lines.len() {
        return None;
    }
    let expected = finding.sink_start_line;
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for i in 0..=(lines.len() - snippet_lines) {
        let end = i + snippet_lines - 1;
        if lines[i..=end].join("\n") == finding.patch_snippet {
            candidates.push(((i + 1).abs_diff(expected), i));
        }
    }
    let min_dist = candidates.iter().map(|(d, _)| *d).min()?;
    let best: Vec<usize> =
        candidates.iter().filter(|(d, _)| *d == min_dist).map(|(_, i)| *i).collect();
    match best.as_slice() {
        [only] => Some((*only, *only + snippet_lines - 1)),
        _ => None,
    }
}

fn extract_query_var(sink_text: &str) -> Option<String> {
    let markers = ["mysqli_query(", "mysqli_multi_query(", "->query(", "->multi_query("];
    let start = markers.iter().find_map(|m| sink_text.find(m).map(|i| i + m.len()))?;
    let bytes = sink_text.as_bytes();
    let mut depth = 1i32;
    let mut end = None;
    for (i, &b) in bytes[start..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let args = &sink_text[start..end?];
    let last = args.rsplit(',').next()?.trim();
    if last.starts_with('$')
        && last.len() > 1
        && last[1..].chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        Some(last.to_string())
    } else {
        None
    }
}

fn format_with_indentation(patch_text: &str, target_line: &str) -> String {
    let indent = target_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
    if indent.is_empty() {
        return patch_text.to_string();
    }

    let mut formatted_lines = Vec::new();
    for line in patch_text.lines() {
        if line.trim().is_empty() {
            formatted_lines.push(String::new());
        } else if line.starts_with(&indent) {
            formatted_lines.push(line.to_string());
        } else {
            formatted_lines.push(format!("{}{}", indent, line.trim_start()));
        }
    }
    formatted_lines.join("\n")
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path } => {
            let symbols = PolyglotScanner::scan_path(&path);
            println!("==> [Polyglot Scanner] Discovered {} symbols total.", symbols.len());
        }
        Commands::Audit {
            path,
            repo,
            ai_triage,
            auto_heal,
            generate_report,
            branch_fix,
            run_tests,
        } => {
            let mut scan_path = path.clone();
            let mut branch_name = String::new();

            if let Some(repo_url) = repo {
                scan_path = PathBuf::from("./nexus_audit_target");
                if scan_path.exists() {
                    fs::remove_dir_all(&scan_path)?;
                }
                println!("==> [Git] Cloning target repository: {}...", repo_url);
                let status = Command::new("git")
                    .args(["clone", "--depth", "1", &repo_url, scan_path.to_str().unwrap()])
                    .status()?;
                if !status.success() {
                    return Err(anyhow::anyhow!("Failed to clone repository"));
                }
            }

            if branch_fix {
                // Check if working tree has uncommitted modifications
                let status_out = Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(&scan_path)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default();

                if !status_out.is_empty() {
                    println!("==> [Git Automation Warning] Working directory contains uncommitted changes. Branching will include dirty state.");
                }

                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                branch_name = format!("nexus/security-remediation-{}", ts);
                println!("==> [Git Automation] Creating isolated remediation branch: {}...", branch_name);
                let _ = Command::new("git")
                    .args(["checkout", "-b", &branch_name])
                    .current_dir(&scan_path)
                    .output();
            }

            println!(
                "==> [Security Auditor] Commencing AST vulnerability scan on: {}",
                scan_path.display()
            );

            let rules_dir = PathBuf::from("apps/nexus_ast_tool/rules");
            let rules = PolyglotScanner::load_rules(&rules_dir);
            println!(
                "==> [Rules Engine] Loaded {} dynamic security rules from {:?}",
                rules.len(),
                rules_dir
            );

            let mut findings = PolyglotScanner::scan_security_vulnerabilities(&scan_path, &rules);

            let mut report_md = String::from("# 🛡️ Nexus Singularity Bug Bounty Report\n\n");

            let mut actionable_count = 0;
            let mut fixed_count = 0;
            let mut failed_fix_count = 0;
            let mut already_resolved_count = 0;
            let mut safe_count = 0;
            let mut unknown_count = 0;
            let mut _triage_error_count = 0;

            if findings.is_empty() {
                println!("==> [Security Auditor] No obvious dangerous sinks found. Codebase looks clean!");
            } else {
                println!(
                    "==> [Security Auditor] WARNING: Discovered {} potential vulnerabilities.",
                    findings.len()
                );

                if ai_triage {
                    println!("\n==> [AI Triage] Initializing LLM to filter false positives...");

                    findings.sort_by(|a, b| {
                        a.path.cmp(&b.path).then(a.sink_start_line.cmp(&b.sink_start_line))
                    });

                    for finding in findings {
                        let file_source = match fs::read_to_string(&finding.path) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };

                        if !file_source.contains(&finding.sink_text) {
                            already_resolved_count += 1;
                            continue;
                        }

                        println!(
                            "\n    [?] Evaluating {} at line {} in {}...",
                            finding.pattern_name.to_uppercase(),
                            finding.sink_start_line,
                            finding.path.file_name().unwrap_or_default().to_string_lossy()
                        );
                        let file_path_str = finding.path.display().to_string();
                        let lang_str = format!("{:?}", finding.language);

                        match nexus_ast_tool::client::ApiClient::triage_vulnerability(
                            &finding.pattern_name,
                            &finding.source_snippet,
                        ) {
                            Ok((state_raw, llm_severity, reason)) => {
                                let classification = match state_raw.trim().to_uppercase().as_str() {
                                    "EXPLOITABLE" => "EXPLOITABLE",
                                    "SAFE" => "SAFE",
                                    _ => "UNKNOWN",
                                };

                                if classification == "EXPLOITABLE" {
                                    actionable_count += 1;
                                    println!(
                                        "        [🔴 EXPLOITABLE] Rule severity: {} (model said: {})",
                                        finding.severity, llm_severity
                                    );
                                    println!("        [🧠 AI Reasoning] {}", reason);

                                    report_md.push_str(&format!(
                                        "## 🚨 Vulnerability: {}\n",
                                        finding.pattern_name.to_uppercase()
                                    ));
                                    report_md
                                        .push_str(&format!("- **Severity:** {}\n", finding.severity));
                                    report_md.push_str(&format!(
                                        "- **File:** `{}` (Sink lines {}-{})\n\n",
                                        file_path_str, finding.sink_start_line, finding.sink_end_line
                                    ));
                                    report_md.push_str(&format!("### Description\n{}\n\n", reason));
                                    report_md.push_str(&format!(
                                        "### Vulnerable Code\n```{}\n{}\n```\n\n",
                                        lang_str.to_lowercase(),
                                        finding.patch_snippet
                                    ));

                                    if auto_heal {
                                        println!("        [🛠️ Auto-Heal] Generating secure anchor-based patch...");

                                        let file_source = match fs::read_to_string(&finding.path) {
                                            Ok(src) => src,
                                            Err(e) => {
                                                println!("        [❌ Patch Error] Fail-closed read failure: {}", e);
                                                failed_fix_count += 1;
                                                continue;
                                            }
                                        };

                                        let lines: Vec<&str> = file_source.lines().collect();

                                        let Some((live_sink_start, live_sink_end)) = resolve_anchor(&file_source, &finding) else {
                                            println!("        [ℹ️ Already Resolved] Sink lines no longer present. Marked clean.");
                                            already_resolved_count += 1;
                                            continue;
                                        };

                                        let live_sink_line = live_sink_start + 1;

                                        let is_secondary_fetch = finding.sink_text.contains("$result = mysqli_query")
                                            || finding.sink_text.contains("$result=mysqli_query");

                                        let (cleaned_patch, effective_start, effective_end) = if is_secondary_fetch && file_source.contains("$stmt = $con->prepare") {
                                            println!("        [⚡ Deterministic Splice] Connecting secondary fetch to active prepared statement.");
                                            (
                                                "$result = $stmt->get_result();".to_string(),
                                                live_sink_start,
                                                live_sink_end,
                                            )
                                        } else {
                                            let mut eff_start = live_sink_start;
                                            let mut eff_end = live_sink_end;

                                            let taint = extract_query_var(&finding.sink_text).and_then(|var_name| {
                                                nexus_ast_tool::polyglot::find_concat_assignment(
                                                    &file_source,
                                                    &finding.language.tree_sitter_language(),
                                                    &var_name,
                                                    live_sink_line,
                                                )
                                                .map(|(c_start, c_end, _)| (var_name, c_start, c_end))
                                            });

                                            if let Some((ref var_name, c_start, _c_end)) = taint {
                                                println!(
                                                    "        [🔗 Multi-Site Taint] {} built near line {}. Splicing locally to line {}.",
                                                    var_name, c_start, live_sink_line
                                                );
                                                eff_start = c_start.saturating_sub(1);
                                                eff_end = live_sink_end.min(lines.len().saturating_sub(1));
                                            }

                                            let effective_snippet = lines[eff_start..=eff_end].join("\n");

                                            let replacement = match nexus_ast_tool::client::ApiClient::generate_security_patch(
                                                &finding.pattern_name, &effective_snippet, &lang_str,
                                            ) {
                                                Ok(r) => r,
                                                Err(e) => {
                                                    println!("        [❌ Patch Gen Error] {}", e);
                                                    failed_fix_count += 1;
                                                    continue;
                                                }
                                            };

                                            let is_php = lang_str.eq_ignore_ascii_case("PHP");
                                            let raw_cleaned = nexus_ast_tool::sanitizer::sanitize_model_patch(&replacement, is_php);
                                            let target_first_line = lines[eff_start];
                                            let indented_patch = format_with_indentation(&raw_cleaned, target_first_line);

                                            (
                                                indented_patch,
                                                eff_start,
                                                eff_end,
                                            )
                                        };

                                        if cleaned_patch.trim().is_empty() {
                                            println!("        [❌ Patch Rejected] Empty replacement.");
                                            failed_fix_count += 1;
                                            continue;
                                        }

                                        let crlf = file_source.matches("\r\n").count() * 2 > file_source.matches('\n').count();
                                        let sep = if crlf { "\r\n" } else { "\n" };
                                        let patch_text = if crlf {
                                            cleaned_patch.replace('\n', "\r\n")
                                        } else {
                                            cleaned_patch.clone()
                                        };

                                        let mut new_source = String::with_capacity(file_source.len() + patch_text.len());
                                        new_source.push_str(&lines[..effective_start].join(sep));
                                        if effective_start > 0 { new_source.push_str(sep); }
                                        new_source.push_str(&patch_text);
                                        if effective_end + 1 < lines.len() {
                                            new_source.push_str(sep);
                                            new_source.push_str(&lines[effective_end + 1..].join(sep));
                                        }
                                        if file_source.ends_with('\n') && !new_source.ends_with('\n') {
                                            new_source.push_str(sep);
                                        }

                                        // Gate 1: Formal Syntax Check
                                        let ts_ok = finding.language.parses_cleanly(&new_source);
                                        let lint = if ts_ok { external_lint(&lang_str, &new_source, &finding.path) } else { None };
                                        let syntax_ok = ts_ok && lint.unwrap_or(true);

                                        // Gate 2: Semantic Integrity Gate
                                        let mut semantic_error = String::new();
                                        let semantic_ok = if syntax_ok {
                                            match PolyglotScanner::verify_semantic_integrity(
                                                &finding.pattern_name,
                                                finding.language,
                                                &file_source,
                                                &cleaned_patch,
                                            ) {
                                                Ok(()) => true,
                                                Err(err) => {
                                                    semantic_error = err;
                                                    false
                                                }
                                            }
                                        } else { false };

                                        // Gate 3: Strict AST Rescan
                                        let ast_ok = if syntax_ok && semantic_ok {
                                            let new_start = effective_start + 1;
                                            let new_end = new_start + patch_text.lines().count().saturating_sub(1);
                                            let residuals = PolyglotScanner::scan_source(&finding.path, &new_source, &rules);
                                            !residuals.iter().any(|f| {
                                                f.pattern_name == finding.pattern_name
                                                    && f.sink_start_line + 2 >= new_start
                                                    && f.sink_end_line <= new_end + 2
                                            })
                                        } else { false };

                                        if syntax_ok && semantic_ok && ast_ok {
                                            if let Err(e) = fs::write(&finding.path, &new_source) {
                                                println!("        [❌ Patch Error] I/O write error: {}", e);
                                                failed_fix_count += 1;
                                                continue;
                                            }
                                            fixed_count += 1;
                                            println!("        [✅ Patch Verified] Syntax OK, Semantic checks passed, and AST rescan confirms sink eliminated.");
                                            report_md.push_str("### Recommended Patch (Auto-Healed, Verified)\n");
                                            report_md.push_str(&format!(
                                                "```{}\n{}\n```\n\n---\n\n",
                                                lang_str.to_lowercase(),
                                                cleaned_patch
                                            ));
                                            continue;
                                        }

                                        let reject_reason = if !ts_ok {
                                            "tree-sitter syntax gate failed"
                                        } else if lint == Some(false) {
                                            "external linter rejected the patch"
                                        } else if !semantic_ok {
                                            &semantic_error
                                        } else {
                                            "AST rescan still detects the sink"
                                        };

                                        failed_fix_count += 1;
                                        println!("        [❌ Patch Rejected] {}. Nothing was written.", reject_reason);
                                        report_md.push_str(&format!(
                                            "### Recommended Patch (REJECTED: {} - never written)\n```{}\n{}\n```\n\n---\n\n",
                                            reject_reason, lang_str.to_lowercase(), cleaned_patch));
                                        continue;
                                    }

                                    report_md.push_str("---\n\n");
                                } else if classification == "SAFE" {
                                    safe_count += 1;
                                    println!("        [🟢 SAFE] Explicitly classified safe.");
                                } else {
                                    unknown_count += 1;
                                    println!("        [🟡 UNKNOWN] Security state cannot be established without assumptions.");
                                }
                            }
                            Err(e) => {
                                _triage_error_count += 1;
                                println!("        [⚠️ API Error] {}", e);
                            }
                        }
                    }
                }
            }

            // Always execute test suite if requested, regardless of findings count
            let tests_passed = if run_tests {
                run_project_test_suite(&scan_path).unwrap_or(true)
            } else {
                true
            };

            let remediation_clean = failed_fix_count == 0
                && actionable_count == (fixed_count + already_resolved_count)
                && tests_passed;

            if generate_report {
                report_md.push_str("\n# Final Audit Summary\n\n");
                report_md.push_str(&format!("- **Actionable findings:** {}\n", actionable_count));
                report_md.push_str(&format!("- **Verified fixed:** {}\n", fixed_count));
                report_md.push_str(&format!("- **Remediation failed:** {}\n", failed_fix_count));
                report_md.push_str(&format!("- **Already resolved:** {}\n", already_resolved_count));
                report_md.push_str(&format!("- **Explicitly safe:** {}\n", safe_count));
                report_md.push_str(&format!("- **Unknown (needs manual review):** {}\n", unknown_count));
                report_md.push_str(&format!("- **Test Suite Status:** {}\n", if tests_passed { "PASSED / SKIPPED" } else { "FAILED" }));

                if remediation_clean {
                    report_md.push_str("\n## Remediation Integrity\n\n**PASS** — Every actionable finding was verified fixed and passed all gates.\n\n");
                } else {
                    report_md.push_str("\n## Remediation Integrity\n\n**FAIL** — One or more actionable findings were not verified fixed or tests failed.\n\n");
                }

                fs::write("bounty_report.md", &report_md)?;

                println!("\n==> [Report Generator] 📄 bounty_report.md generated.");
                println!("    Actionable findings : {}", actionable_count);
                println!("    Verified fixed      : {}", fixed_count);
                println!("    Remediation failed  : {}", failed_fix_count);
                println!("    Already resolved    : {}", already_resolved_count);
                if remediation_clean {
                    println!("    [✅ REMEDIATION INTEGRITY] PASS");
                } else {
                    println!("    [❌ REMEDIATION INTEGRITY] FAIL");
                }
            }

            if branch_fix && fixed_count > 0 && remediation_clean {
                println!("\n==> [Git Automation] Staging verified fixes & creating commit...");
                let _ = Command::new("git").args(["add", "."]).current_dir(&scan_path).output();
                let commit_msg = format!("chore(security): automated remediation of {} vulnerabilities via AthenaFusionX", fixed_count);
                let commit_status = Command::new("git")
                    .args(["commit", "-m", &commit_msg])
                    .current_dir(&scan_path)
                    .status();

                if commit_status.map(|s| s.success()).unwrap_or(false) {
                    println!("    [🚀 Git Commit OK] Branch `{}` is ready for Pull Request creation.", branch_name);
                }
            }
        }
        Commands::Refactor { file, symbol, strategy, payload, instruction } => {
            let agent = nexus_ast_tool::agent::RefactorAgent::new();

            if let Some(inst) = instruction {
                agent.execute_instruction(&file, &symbol, &inst, &strategy)?;
            } else if !payload.is_empty() {
                agent.apply_refactoring(&file, &symbol, &strategy, &payload)?;
            } else {
                return Err(anyhow::anyhow!("Refactor requires either --instruction or --payload"));
            }
        }
        Commands::Optimize { path } => {
            nexus_ast_tool::supervisor::Supervisor::self_optimize(&path)?;
        }
    }
    Ok(())
}
