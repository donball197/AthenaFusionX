use anyhow::Result;
use clap::{Parser, Subcommand};
use nexus_ast_tool::polyglot::{PolyglotScanner, SecurityFinding};
use nexus_ast_tool::sandbox::{verify_poc_against_patch, patch_static_issues, HealSandbox};
use nexus_ast_tool::sarif::{render_json, render_sarif, AuditFindingRecord};
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
        sandbox_verify: bool,
        #[arg(long)]
        poc_verify: bool,
        #[arg(long)]
        generate_report: bool,
        #[arg(long, default_value = "markdown", help = "Report format: markdown | sarif | json")]
        format: String,
        #[arg(long, help = "Optional path to write SARIF 2.1.0 report")]
        sarif_out: Option<PathBuf>,
        #[arg(long, help = "Optional path to write JSON report")]
        json_out: Option<PathBuf>,
        #[arg(long)]
        branch_fix: bool,
        #[arg(long)]
        run_tests: bool,
        #[arg(long, help = "Hold verified patches for human promotion instead of auto-promoting (needs --auto-heal --sandbox-verify)")]
        hold: bool,
        #[arg(long, default_value = ".nexus/holds")]
        hold_dir: PathBuf,
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
    Study {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        #[arg(
            long,
            default_value = "nexus_study_report",
            help = "Directory for the generated study report"
        )]
        output: PathBuf,
    },
    Approve {
        #[arg(long)]
        candidate_id: String,

        #[arg(long)]
        report_identity: String,

        #[arg(long, help = "Required explicit human authorization token")]
        confirm: String,

        #[arg(
            long,
            default_value = ".nexus/evolution/approvals",
            help = "Directory where the approval artifact is persisted"
        )]
        output: PathBuf,
    },
    Tui {
        #[arg(long, default_value = ".nexus/holds")]
        dir: PathBuf,
    },
    Review {
        #[arg(long, help = "Show one hold; omit to list all")]
        hold: Option<String>,
        #[arg(long, default_value = ".nexus/holds")]
        dir: PathBuf,
    },
    Promote {
        #[arg(long)]
        hold: String,
        #[arg(long, default_value = ".nexus/holds")]
        dir: PathBuf,
    },
    Optimize {
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        #[arg(
            long,
            default_value = "/tmp/nexus_evolution_reports",
            help = "Directory where the immutable optimization report is persisted"
        )]
        output: PathBuf,
    },

    DryRun {
        #[arg(long, help = "Persisted optimization_report.json from the report-only phase")]
        report: PathBuf,

        #[arg(long, help = "Human Approval #1 artifact authorizing this exact report/candidate")]
        approval: PathBuf,

        #[arg(
            long,
            default_value = "/tmp/nexus_evolution_dry_run",
            help = "External directory used for the isolated dry-run workspace and report"
        )]
        output: PathBuf,
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
    println!(
        "==> [Test Suite Gate] Auto-detecting project test harness in {}...",
        repo_path.display()
    );

    let (program, args): (&str, Vec<&str>) = if repo_path.join("Cargo.toml").exists() {
        ("cargo", vec!["test", "--color", "never"])
    } else if repo_path.join("phpunit.xml").exists() || repo_path.join("phpunit.xml.dist").exists()
    {
        ("vendor/bin/phpunit", vec![])
    } else if repo_path.join("pytest.ini").exists()
        || (repo_path.join("tests").exists() && repo_path.join("requirements.txt").exists())
    {
        ("pytest", vec![])
    } else if repo_path.join("go.mod").exists() {
        ("go", vec!["test", "./..."])
    } else if repo_path.join("package.json").exists() {
        ("npm", vec!["test", "--", "--passWithNoTests"])
    } else {
        println!(
            "    [ℹ️ Test Harness] No standard test harness detected (Cargo, PHPUnit, PyTest, Jest). Skipping."
        );
        return None;
    };

    println!("    [🧪 Running] {} {}", program, args.join(" "));
    let output = Command::new(program).args(&args).current_dir(repo_path).output();

    match output {
        Ok(o) => {
            if o.status.success() {
                println!("    [✅ Test Suite Passed] 0 regressions detected.");
                Some(true)
            } else {
                let err_msg = String::from_utf8_lossy(&o.stderr);
                println!(
                    "    [❌ Test Suite Failed]\n{}",
                    err_msg.lines().take(10).collect::<Vec<_>>().join("\n")
                );
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
            sandbox_verify,
            poc_verify,
            generate_report,
            format,
            sarif_out,
            json_out,
            branch_fix,
            run_tests,
            hold,
            hold_dir,
        } => {
            if hold && !(auto_heal && sandbox_verify) {
                return Err(anyhow::anyhow!("--hold requires --auto-heal and --sandbox-verify"));
            }
            let mut scan_path = path.clone();
            let mut branch_name = String::new();

            if let Some(repo_url) = repo {
                scan_path = std::env::temp_dir().join(format!("nexus_audit_target_{}", std::process::id()));
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
                    println!(
                        "==> [Git Automation Warning] Working directory contains uncommitted changes. Branching will include dirty state."
                    );
                }

                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                branch_name = format!("nexus/security-remediation-{}", ts);
                println!(
                    "==> [Git Automation] Creating isolated remediation branch: {}...",
                    branch_name
                );
                let _ = Command::new("git")
                    .args(["checkout", "-b", &branch_name])
                    .current_dir(&scan_path)
                    .output();
            }

            println!(
                "==> [Security Auditor] Commencing AST vulnerability scan on: {}",
                scan_path.display()
            );

            let rules_dir = {
                let cwd_rules = PathBuf::from("apps/nexus_ast_tool/rules");
                if cwd_rules.exists() {
                    cwd_rules
                } else {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules")
                }
            };
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
            let mut unconfirmed_count: usize = 0;
            let mut already_resolved_count = 0;
            let mut safe_count = 0;
            let mut unknown_count = 0;
            let mut _triage_error_count = 0;
            let mut audit_records: Vec<AuditFindingRecord> = Vec::new();
            let heal_sandbox = if auto_heal && sandbox_verify {
                let sb = HealSandbox::create(&scan_path)?;
                println!(
                    "==> [Sandbox] Isolated remediation workspace: {}",
                    sb.sandbox_root.display()
                );
                Some(sb)
            } else {
                None
            };
            let mut sandbox_modified_files: Vec<PathBuf> = Vec::new();

            if findings.is_empty() {
                println!(
                    "==> [Security Auditor] No obvious dangerous sinks found. Codebase looks clean!"
                );
            } else {
                println!(
                    "==> [Security Auditor] WARNING: Discovered {} potential vulnerabilities.",
                    findings.len()
                );

                if ai_triage || auto_heal {
                    println!("\n==> [Processing] Analyzing {} finding(s)...", findings.len());

                    findings.sort_by(|a, b| {
                        a.path.cmp(&b.path).then(a.sink_start_line.cmp(&b.sink_start_line))
                    });

                    let mut rejected_with_reason: Vec<(String, String)> = Vec::new();
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

                        let triage_result: Result<(String, String, String), String> = if ai_triage {
                            let triage_snippet = if finding.taint_path.is_empty() {
                                finding.source_snippet.clone()
                            } else {
                                format!(
                                    "{}\n// Static taint trace: {}",
                                    finding.source_snippet,
                                    finding.taint_path.join(" -> ")
                                )
                            };
                            nexus_ast_tool::client::ApiClient::triage_vulnerability(
                                &finding.pattern_name,
                                &triage_snippet,
                            ).map_err(|e| e.to_string())
                        } else {
                            Ok(("EXPLOITABLE".to_string(), finding.severity.clone(), "Auto-heal mode - no LLM triage".to_string()))
                        };
                        match triage_result {
                            Ok((state_raw, llm_severity, reason)) => {
                                let classification = match state_raw.trim().to_uppercase().as_str()
                                {
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
                                    report_md.push_str(&format!(
                                        "- **Severity:** {}\n",
                                        finding.severity
                                    ));
                                    report_md.push_str(&format!(
                                        "- **File:** `{}` (Sink lines {}-{})\n\n",
                                        file_path_str,
                                        finding.sink_start_line,
                                        finding.sink_end_line
                                    ));
                                    report_md.push_str(&format!("### Description\n{}\n\n", reason));
                                    report_md.push_str(&format!(
                                        "### Vulnerable Code\n```{}\n{}\n```\n\n",
                                        lang_str.to_lowercase(),
                                        finding.patch_snippet
                                    ));

                                    if auto_heal {
                                        println!(
                                            "        [🛠️ Auto-Heal] Generating secure anchor-based patch..."
                                        );

                                        let edit_path = match &heal_sandbox {
                                            Some(sb) => sb.sandbox_path(&finding.path)?,
                                            None => finding.path.clone(),
                                        };

                                        let file_source = match fs::read_to_string(&edit_path) {
                                            Ok(src) => src,
                                            Err(e) => {
                                                println!(
                                                    "        [❌ Patch Error] Fail-closed read failure: {}",
                                                    e
                                                );
                                                failed_fix_count += 1;
                                                continue;
                                            }
                                        };

                                        let lines: Vec<&str> = file_source.lines().collect();

                                        let Some((live_sink_start, live_sink_end)) =
                                            resolve_anchor(&file_source, &finding)
                                        else {
                                            println!(
                                                "        [ℹ️ Already Resolved] Sink lines no longer present. Marked clean."
                                            );
                                            already_resolved_count += 1;
                                            continue;
                                        };

                                        let live_sink_line = live_sink_start + 1;

                                        let is_secondary_fetch = finding
                                            .sink_text
                                            .contains("$result = mysqli_query")
                                            || finding.sink_text.contains("$result=mysqli_query");

                                        let has_prior_prepare = lines[..live_sink_start].iter().any(|l| {
                                            l.contains("$stmt = $con->prepare")
                                                || l.contains("$stmt=$con->prepare")
                                        });
                                        let (cleaned_patch, effective_start, effective_end) =
                                            if is_secondary_fetch && has_prior_prepare {
                                                println!(
                                                    "        [⚡ Deterministic Splice] Connecting secondary fetch to active prepared statement."
                                                );
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

                                                if let Some((ref var_name, c_start, _c_end)) = taint
                                                {
                                                    println!(
                                                        "        [🔗 Multi-Site Taint] {} built near line {}. Splicing locally to line {}.",
                                                        var_name, c_start, live_sink_line
                                                    );
                                                    eff_start = c_start.saturating_sub(1);
                                                    eff_end = live_sink_end
                                                        .min(lines.len().saturating_sub(1));
                                                }

                                                if taint.is_none() {
                                                    if let Some((stmt_start, stmt_end)) =
                                                        finding.language.enclosing_statement_lines(
                                                            &finding.path,
                                                            &file_source,
                                                            eff_start + 1,
                                                            eff_end + 1,
                                                        )
                                                    {
                                                        eff_start = stmt_start.saturating_sub(1);
                                                        eff_end = stmt_end
                                                            .saturating_sub(1)
                                                            .min(lines.len().saturating_sub(1));
                                                    }
                                                }

                                                // Expand 2-line builder + sink spans for Rust/Go/Zig
                                                if finding.pattern_name == "RUST_SQL_INJECTION" {
                                                    let head_line = lines[eff_start].trim();
                                                    let cur_block = lines[eff_start..=eff_end].join("\n");
                                                    if head_line.starts_with("let ") && cur_block.contains("format!") {
                                                        let after_let = head_line.trim_start_matches("let ").trim_start_matches("mut ").trim();
                                                        let var_name = after_let
                                                            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                                                            .next()
                                                            .unwrap_or("");
                                                        if !var_name.is_empty() && eff_end + 1 < lines.len() && lines[eff_end + 1].contains(var_name) {
                                                            if let Some((_, next_end)) = finding.language.enclosing_statement_lines(
                                                                &finding.path,
                                                                &file_source,
                                                                eff_end + 2,
                                                                eff_end + 2,
                                                            ) {
                                                                eff_end = next_end.saturating_sub(1).min(lines.len().saturating_sub(1));
                                                            } else {
                                                                eff_end = (eff_end + 1).min(lines.len().saturating_sub(1));
                                                            }
                                                        }
                                                    }
                                                } else if matches!(finding.pattern_name.as_str(), "GO_SQL_INJECTION" | "GO_COMMAND_INJECTION") && eff_start > 0 {
                                                    let prev = lines[eff_start - 1].trim();
                                                    if prev.contains("fmt.Sprintf(") {
                                                        if let Some(lhs) = prev.split(":=").next().or_else(|| prev.split('=').next()) {
                                                            let var_name = lhs.trim();
                                                            if !var_name.is_empty()
                                                                && var_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                                                                && lines[eff_start..=eff_end].join("\n").contains(var_name)
                                                            {
                                                                eff_start -= 1;
                                                            }
                                                        }
                                                    }
                                                } else if finding.pattern_name == "ZIG_COMMAND_INJECTION" && eff_start > 0 {
                                                    let prev = lines[eff_start - 1].trim();
                                                    if (prev.starts_with("const ") || prev.starts_with("var ")) && prev.contains('=') {
                                                        let decl = prev.trim_start_matches("const ").trim_start_matches("var ").trim();
                                                        let var_name = decl
                                                            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                                                            .next()
                                                            .unwrap_or("");
                                                        if !var_name.is_empty() && lines[eff_start..=eff_end].join("\n").contains(var_name) {
                                                            eff_start -= 1;
                                                        }
                                                    }
                                                }

                                                let effective_snippet =
                                                    lines[eff_start..=eff_end].join("\n");

                                                let header_imports: String = lines
                                                    .iter()
                                                    .take(30)
                                                    .copied()
                                                    .collect::<Vec<_>>()
                                                    .join("\n");
                                                let context_snippet = format!(
                                                    "{}\n\n// ... enclosing function context ...\n{}",
                                                    header_imports, finding.source_snippet
                                                );

                                                let mut replacement = String::new();
                                                let mut gen_err: Option<String> = None;
                                                let mut feedback = String::new();
                                                for attempt in 0..2 {
                                                    let ctx = if feedback.is_empty() {
                                                        context_snippet.clone()
                                                    } else {
                                                        format!("{}\n\nPREVIOUS ATTEMPT REJECTED: {}\nFix this. Keep identifiers and table-name expressions exactly as in the snippet; never hardcode names.", context_snippet, feedback)
                                                    };
                                                    match nexus_ast_tool::client::ApiClient::generate_security_patch_with_context(
                                                        &finding.pattern_name, &effective_snippet, &ctx, &lang_str,
                                                    ) {
                                                        Ok(r) => replacement = r,
                                                        Err(e) => {
                                                            gen_err = Some(e.to_string());
                                                            break;
                                                        }
                                                    }
                                                    if attempt == 1 {
                                                        break;
                                                    }
                                                    let is_php_retry = lang_str.eq_ignore_ascii_case("PHP");
                                                    let cand_raw = nexus_ast_tool::sanitizer::sanitize_model_patch(&replacement, is_php_retry);
                                                    let cand = format_with_indentation(&cand_raw, lines[eff_start]);
                                                    let mut cs = lines[..eff_start].join("\n");
                                                    cs.push('\n');
                                                    cs.push_str(&cand);
                                                    if eff_end + 1 < lines.len() {
                                                        cs.push('\n');
                                                        cs.push_str(&lines[eff_end + 1..].join("\n"));
                                                    }
                                                    let mut issues = patch_static_issues(&file_source, &cs);
                                                    // Auto-inject missing Python imports
                                                    if finding.pattern_name.contains("PYTHON") && issues.iter().any(|i| i.contains("pyflakes: undefined name")) {
                                                        let mut imports_to_add: Vec<&str> = Vec::new();
                                                        for iss in &issues {
                                                            if let Some(rest) = iss.strip_prefix("pyflakes: undefined name '") {
                                                                if let Some(name) = rest.split('\'').next() {
                                                                    match name {
                                                                        "ast" => imports_to_add.push("import ast"),
                                                                        "json" => imports_to_add.push("import json"),
                                                                        "escape" => imports_to_add.push("from markupsafe import escape"),
                                                                        "html" => imports_to_add.push("import html"),
                                                                        "shlex" => imports_to_add.push("import shlex"),
                                                                        _ => {}
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if !imports_to_add.is_empty() {
                                                            let imp = imports_to_add.join("\n");
                                                            issues.retain(|i| !i.starts_with("pyflakes:"));
                                                            issues.push(format!("auto_import: {}", imp.replace('\n', "; ")));
                                                        }
                                                    }
                                                    if let Err(sem_err) = PolyglotScanner::verify_semantic_integrity(
                                                        &finding.pattern_name,
                                                        finding.language,
                                                        &file_source,
                                                        &cand,
                                                    ) {
                                                        issues.push(sem_err);
                                                    }
                                                    if cand.trim_start().starts_with('.') && !effective_snippet.trim_start().starts_with('.') {
                                                        issues.push("patch starts with '.' and dropped the receiver/query call - keep the full call expression".to_string());
                                                    }
                                                    if finding.pattern_name == "SQL_INJECTION"
                                                        && matches!(finding.language, nexus_ast_tool::polyglot::SupportedLanguage::JavaScript | nexus_ast_tool::polyglot::SupportedLanguage::TypeScript)
                                                        && (effective_snippet.contains(".query(") || effective_snippet.contains(".execute("))
                                                        && !cand.contains(".query(")
                                                        && !cand.contains(".execute(")
                                                    {
                                                        issues.push("patch dropped the database .query()/.execute() call instead of parameterizing it".to_string());
                                                    }
                                                    let has_code_line = cand.lines().any(|l| {
                                                        let t = l.trim();
                                                        !t.is_empty()
                                                            && !t.starts_with("//")
                                                            && !t.starts_with('#')
                                                            && !t.starts_with("/*")
                                                            && !t.starts_with('*')
                                                            && !t.starts_with("*/")
                                                    });
                                                    if !has_code_line {
                                                        issues.push("patch contains only comments and no executable code".to_string());
                                                    }
                                                    // Go SQLi: fmt.Sprintf must be removed for parameterized fix
                                                    if finding.pattern_name == "GO_SQL_INJECTION"
                                                        && cand.contains("fmt.Sprintf")
                                                        && cand.contains("db.Query")
                                                    {
                                                        issues.push("fmt.Sprintf query construction still present - replace ALL query-building lines with ONE parameterized db.Query using ? placeholder".to_string());
                                                    }
                                                    // Go CMDi: bare variable to exec.Command needs allowlist
                                                    if finding.pattern_name == "GO_COMMAND_INJECTION" {
                                                        if let Some(pos) = cand.find("exec.Command(") {
                                                            let ac = &cand[pos + "exec.Command(".len()..];
                                                            let tc = ac.trim_start();
                                                            if tc.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
                                                                if !cand.contains("map[string]bool{")
                                                                    && !cand.contains("allowed :=")
                                                                    && !cand.contains("allowed=")
                                                                {
                                                                    issues.push("exec.Command with bare variable and no allowlist definition - add allowed := map[string]bool{...} before the call".to_string());
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // Python SQLi: f-strings must be replaced
                                                    if finding.pattern_name == "SQL_INJECTION"
                                                        && lang_str.eq_ignore_ascii_case("python")
                                                        && (cand.contains("f\"") || cand.contains("f'"))
                                                        && (cand.contains("SELECT") || cand.contains("INSERT") || cand.contains("UPDATE") || cand.contains("DELETE"))
                                                    {
                                                        issues.push("SQL f-string interpolation still present - use parameterized execute with bound parameters".to_string());
                                                    }
                                                    if issues.is_empty() {
                                                        break;
                                                    }
                                                    feedback = issues.join("; ");
                                                    println!("        [🔁 Retry] Static check failed ({}). Regenerating once.", feedback);
                                                }
                                                if let Some(e) = gen_err {
                                                    println!("        [❌ Patch Gen Error] {}", e);
                                                    failed_fix_count += 1;
                                                    continue;
                                                }

                                                let is_php = lang_str.eq_ignore_ascii_case("PHP");
                                                let raw_cleaned =
                                                    nexus_ast_tool::sanitizer::sanitize_model_patch(
                                                        &replacement,
                                                        is_php,
                                                    );
                                                let target_first_line = lines[eff_start];
                                                let indented_patch = format_with_indentation(
                                                    &raw_cleaned,
                                                    target_first_line,
                                                );

                                                (indented_patch, eff_start, eff_end)
                                            };

                                        let has_exec_line = cleaned_patch.lines().any(|l| {
                                            let t = l.trim();
                                            !t.is_empty()
                                                && !t.starts_with("//")
                                                && !t.starts_with('#')
                                                && !t.starts_with("/*")
                                                && !t.starts_with('*')
                                                && !t.starts_with("*/")
                                        });
                                        if cleaned_patch.trim().is_empty() || !has_exec_line {
                                            println!(
                                                "        [❌ Patch Rejected] Empty or comment-only replacement."
                                            );
                                            failed_fix_count += 1;
                                            continue;
                                        }
                                        if cleaned_patch.contains("\"payload\"") {
                                            println!(
                                                "        [❌ Patch Rejected] Unwrapped JSON payload marker detected."
                                            );
                                            failed_fix_count += 1;
                                            continue;
                                        }
                                        let orig_slice = lines[effective_start..=effective_end].join("\n");
                                        if cleaned_patch.trim_start().starts_with('.') && !orig_slice.trim_start().starts_with('.') {
                                            println!(
                                                "        [❌ Patch Rejected] Patch starts with '.' and dropped the receiver/query call."
                                            );
                                            failed_fix_count += 1;
                                            continue;
                                        }
                                        if finding.pattern_name == "SQL_INJECTION"
                                            && matches!(finding.language, nexus_ast_tool::polyglot::SupportedLanguage::JavaScript | nexus_ast_tool::polyglot::SupportedLanguage::TypeScript)
                                            && (orig_slice.contains(".query(") || orig_slice.contains(".execute("))
                                            && !cleaned_patch.contains(".query(")
                                            && !cleaned_patch.contains(".execute(")
                                        {
                                            println!(
                                                "        [❌ Patch Rejected] Patch dropped the database .query()/.execute() call."
                                            );
                                            failed_fix_count += 1;
                                            continue;
                                        }

                                        let crlf = file_source.matches("\r\n").count() * 2
                                            > file_source.matches('\n').count();
                                        let sep = if crlf { "\r\n" } else { "\n" };
                                        let patch_text = if crlf {
                                            cleaned_patch.replace('\n', "\r\n")
                                        } else {
                                            cleaned_patch.clone()
                                        };

                                        let mut new_source = String::with_capacity(
                                            file_source.len() + patch_text.len(),
                                        );
                                        new_source.push_str(&lines[..effective_start].join(sep));
                                        if effective_start > 0 {
                                            new_source.push_str(sep);
                                        }
                                        new_source.push_str(&patch_text);
                                        if effective_end + 1 < lines.len() {
                                            new_source.push_str(sep);
                                            new_source
                                                .push_str(&lines[effective_end + 1..].join(sep));
                                        }
                                        if file_source.ends_with('\n')
                                            && !new_source.ends_with('\n')
                                        {
                                            new_source.push_str(sep);
                                        }

                                        // Gate 1: Formal Syntax Check (differential when original file has pre-existing syntax errors)
                                        let orig_errors = finding
                                            .language
                                            .tree_sitter_error_count_for_path(&finding.path, &file_source);
                                        let new_errors = finding
                                            .language
                                            .tree_sitter_error_count_for_path(&finding.path, &new_source);
                                        let ts_ok = match (orig_errors, new_errors) {
                                            (Some(0), Some(0)) => true,
                                            (Some(oe), Some(ne)) if oe > 0 && ne <= oe => {
                                                finding.language.parses_cleanly_for_path(&finding.path, &cleaned_patch)
                                            }
                                            _ => false,
                                        };
                                        let lint = if ts_ok && orig_errors == Some(0) {
                                            external_lint(&lang_str, &new_source, &finding.path)
                                        } else {
                                            None
                                        };
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
                                        } else {
                                            false
                                        };

                                        // Gate 3: Strict AST Rescan
                                        let ast_ok = if syntax_ok && semantic_ok {
                                            let new_start = effective_start + 1;
                                            let new_end = new_start
                                                + patch_text.lines().count().saturating_sub(1);
                                            let residuals = PolyglotScanner::scan_source(
                                                &finding.path,
                                                &new_source,
                                                &rules,
                                            );
                                            !residuals.iter().any(|f| {
                                                f.pattern_name == finding.pattern_name
                                                    && f.sink_start_line + 2 >= new_start
                                                    && f.sink_end_line <= new_end + 2
                                            })
                                        } else {
                                            false
                                        };

                                        let mut static_error = String::new();
                                        let static_ok = if syntax_ok
                                            && semantic_ok
                                            && ast_ok
                                            && lang_str.eq_ignore_ascii_case("python")
                                        {
                                            let mut issues = patch_static_issues(&file_source, &new_source);
                                            if cleaned_patch.contains("json.loads") {
                                                let mut from = 0usize;
                                                while let Some(pos) = new_source[from..].find("dumps(") {
                                                    let abs = from + pos;
                                                    if !new_source[..abs].ends_with("json.") {
                                                        issues.push("deserializer swapped but a non-json dumps() write site remains".to_string());
                                                        break;
                                                    }
                                                    from = abs + 6;
                                                }
                                            }
                                            if issues.is_empty() {
                                                true
                                            } else {
                                                static_error = issues.join("; ");
                                                false
                                            }
                                        } else {
                                            true
                                        };

                                        if syntax_ok && semantic_ok && ast_ok && static_ok {
                                            let poc_result = if poc_verify {
                                                let outcome = verify_poc_against_patch(
                                                    &finding,
                                                    &lines[effective_start.saturating_sub(5)..=(effective_end + 5).min(lines.len().saturating_sub(1))].join("\n"),
                                                    &cleaned_patch,
                                                );
                                                println!("        [🧪 PoC Gate] {}", outcome.detail);
                                                match (outcome.executed, outcome.exploited_before, outcome.blocked_after) {
                                                    (true, Some(true), Some(true)) => Some(true),
                                                    (true, Some(true), Some(false)) => Some(false),
                                                    _ => None,
                                                }
                                            } else {
                                                None
                                            };

                                            if poc_strict_mode() && poc_verify && poc_result.is_none() {
                                                unconfirmed_count += 1;
                                                println!("        [⚠️ Patch Unconfirmed] PoC did not demonstrate exploit-then-block. Nothing was written.");
                                                audit_records.push(AuditFindingRecord {
                                                    finding: finding.clone(),
                                                    triage_state: classification.to_string(),
                                                    triage_reason: Some(reason.clone()),
                                                    model_severity: Some(llm_severity.clone()),
                                                    remediation_status: "UNCONFIRMED".to_string(),
                                                    remediation_detail: Some(cleaned_patch.clone()),
                                                    poc_verified: None,
                                                });
                                                continue;
                                            }

                                            if poc_strict_mode() && poc_result == Some(false) {
                                                failed_fix_count += 1;
                                                println!("        [❌ Patch Rejected] PoC verification failed.");
                                                audit_records.push(AuditFindingRecord {
                                                    finding: finding.clone(),
                                                    triage_state: classification.to_string(),
                                                    triage_reason: Some(reason.clone()),
                                                    model_severity: Some(llm_severity.clone()),
                                                    remediation_status: "REJECTED".to_string(),
                                                    remediation_detail: Some("PoC verification failed".to_string()),
                                                    poc_verified: poc_result,
                                                });
                                                continue;
                                            }

                                            if let Err(e) = fs::write(&edit_path, &new_source) {
                                                println!(
                                                    "        [❌ Patch Error] I/O write error: {}",
                                                    e
                                                );
                                                failed_fix_count += 1;
                                                continue;
                                            }
                                            if heal_sandbox.is_some()
                                                && !sandbox_modified_files.contains(&finding.path)
                                            {
                                                sandbox_modified_files.push(finding.path.clone());
                                            }
                                            fixed_count += 1;
                                            println!(
                                                "        [✅ Patch Accepted] Syntax OK, semantic checks passed, AST rescan confirms sink eliminated. Runtime behavior NOT verified (runtime unverified)."
                                            );
                                            report_md.push_str(
                                                "### Recommended Patch (Auto-Healed, Verified)\n",
                                            );
                                            report_md.push_str(&format!(
                                                "```{}\n{}\n```\n\n---\n\n",
                                                lang_str.to_lowercase(),
                                                cleaned_patch
                                            ));
                                            audit_records.push(AuditFindingRecord {
                                                finding: finding.clone(),
                                                triage_state: classification.to_string(),
                                                triage_reason: Some(reason.clone()),
                                                model_severity: Some(llm_severity.clone()),
                                                remediation_status: "FIXED".to_string(),
                                                remediation_detail: Some(cleaned_patch.clone()),
                                                poc_verified: poc_result,
                                            });
                                            continue;
                                        }

                                        let reject_reason = if !ts_ok {
                                            "tree-sitter syntax gate failed"
                                        } else if lint == Some(false) {
                                            "external linter rejected the patch"
                                        } else if !semantic_ok { &semantic_error } else if !static_ok { &static_error } else {
                                            "AST rescan still detects the sink"
                                        };

                                        failed_fix_count += 1;
                                        println!(
                                            "        [❌ Patch Rejected] {}. Nothing was written.",
                                            reject_reason
                                        );
                                        report_md.push_str(&format!(
                                            "### Recommended Patch (REJECTED: {} - never written)\n```{}\n{}\n```\n\n---\n\n",
                                            reject_reason, lang_str.to_lowercase(), cleaned_patch));
                                        audit_records.push(AuditFindingRecord {
                                            finding: finding.clone(),
                                            triage_state: classification.to_string(),
                                            triage_reason: Some(reason.clone()),
                                            model_severity: Some(llm_severity.clone()),
                                            remediation_status: "REJECTED".to_string(),
                                            remediation_detail: Some(reject_reason.to_string()),
                                            poc_verified: None,
                                        });
                                        continue;
                                    }

                                    audit_records.push(AuditFindingRecord {
                                        finding: finding.clone(),
                                        triage_state: classification.to_string(),
                                        triage_reason: Some(reason.clone()),
                                        model_severity: Some(llm_severity.clone()),
                                        remediation_status: "NOT_ATTEMPTED".to_string(),
                                        remediation_detail: None,
                                        poc_verified: None,
                                    });
                                    report_md.push_str("---\n\n");
                                } else if classification == "SAFE" {
                                    safe_count += 1;
                                    println!("        [🟢 SAFE] Explicitly classified safe.");
                                    audit_records.push(AuditFindingRecord {
                                        finding: finding.clone(),
                                        triage_state: "SAFE".to_string(),
                                        triage_reason: Some(reason.clone()),
                                        model_severity: Some(llm_severity.clone()),
                                        remediation_status: "NOT_ATTEMPTED".to_string(),
                                        remediation_detail: None,
                                        poc_verified: None,
                                    });
                                } else {
                                    unknown_count += 1;
                                    println!(
                                        "        [🟡 UNKNOWN] Security state cannot be established without assumptions."
                                    );
                                    audit_records.push(AuditFindingRecord {
                                        finding: finding.clone(),
                                        triage_state: "UNKNOWN".to_string(),
                                        triage_reason: Some(reason.clone()),
                                        model_severity: Some(llm_severity.clone()),
                                        remediation_status: "NOT_ATTEMPTED".to_string(),
                                        remediation_detail: None,
                                        poc_verified: None,
                                    });
                                }
                            }
                            Err(e) => {
                                _triage_error_count += 1;
                                println!("        [⚠️ API Error] {}", e);
                            }
                        }
                    }
                } else if !findings.is_empty() {
                            eprintln!(">>> [WARN] AUTO-HEAL REQUIRES AI TRIAGE: {} finding(s) skipped. Add --ai-triage to classify and heal.", findings.len());
                    unknown_count = findings.len();
                    for finding in &findings {
                        report_md.push_str(&format!(
                            "## ⚠️ Potential Vulnerability (untriaged): {}\n",
                            finding.pattern_name.to_uppercase()
                        ));
                        report_md.push_str(&format!("- **Rule severity:** {}\n", finding.severity));
                        report_md.push_str(&format!(
                            "- **File:** `{}` (Sink lines {}-{})\n\n",
                            finding.path.display(),
                            finding.sink_start_line,
                            finding.sink_end_line
                        ));
                        report_md.push_str(&format!(
                            "### Vulnerable Code\n```{}\n{}\n```\n\n",
                            format!("{:?}", finding.language).to_lowercase(),
                            finding.patch_snippet
                        ));
                        if !finding.taint_path.is_empty() {
                            report_md.push_str(&format!(
                                "- **Taint Trace:** `{}`\n\n",
                                finding.taint_path.join(" -> ")
                            ));
                        }
                        report_md.push_str(
                            "> Not evaluated by AI triage — rerun with `--ai-triage` to classify.\n\n---\n\n",
                        );
                        audit_records.push(AuditFindingRecord {
                            finding: finding.clone(),
                            triage_state: "UNTRIAGED".to_string(),
                            triage_reason: None,
                            model_severity: None,
                            remediation_status: "NOT_ATTEMPTED".to_string(),
                            remediation_detail: None,
                            poc_verified: None,
                        });
                    }
                }
            }

            // Run tests inside the sandbox first (when --sandbox-verify is enabled) before promoting
            let test_target_root = heal_sandbox
                .as_ref()
                .map(|sb| sb.sandbox_root.clone())
                .unwrap_or_else(|| scan_path.clone());
            let tests_passed = if run_tests {
                run_project_test_suite(&test_target_root).unwrap_or(true)
            } else {
                true
            };

            let remediation_clean = actionable_count > 0 && failed_fix_count == 0 && unconfirmed_count == 0
                && actionable_count == (fixed_count + already_resolved_count)
                && tests_passed;

            eprintln!(">>> [DEBUG] tests_passed={} fixed_count={} modified_files={} hold={}", tests_passed, fixed_count, sandbox_modified_files.len(), hold);
            eprintln!(">>> [DEBUG] failed_fix_count={} unconfirmed_count={} actionable={}", failed_fix_count, unconfirmed_count, actionable_count);
            for (i, rec) in audit_records.iter().enumerate() {
                eprintln!(">>> [DEBUG] rec[{}] pattern={} status={} path={}", i, rec.finding.pattern_name, rec.remediation_status, rec.finding.path.display());
            }

            if let Some(ref sb) = heal_sandbox {
                if tests_passed && fixed_count > 0 && !sandbox_modified_files.is_empty() && hold {
                    let entries = sandbox_modified_files
                        .iter()
                        .map(|p| {
                            let fl: Vec<nexus_ast_tool::hold::HeldFinding> = audit_records
                                .iter()
                                .filter(|r| r.remediation_status == "FIXED" && &r.finding.path == p)
                                .map(|r| nexus_ast_tool::hold::HeldFinding {
                                    pattern: r.finding.pattern_name.clone(),
                                    line: r.finding.sink_start_line,
                                    patch: r.remediation_detail.clone().unwrap_or_default(),
                                })
                                .collect();
                            Ok((p.clone(), sb.sandbox_path(p)?, fl))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let h = nexus_ast_tool::hold::create(&hold_dir, &sb.authoritative_root, entries)?;
                    println!(
                        "==> [Hold] {} file(s) held as {}. Nothing written to target. Review: nexus_ast_tool review --hold {}  |  Apply: nexus_ast_tool promote --hold {}",
                        h.files.len(), h.id, h.id, h.id
                    );
                } else if tests_passed && fixed_count > 0 && !sandbox_modified_files.is_empty() {
                    let count = sb.promote(&sandbox_modified_files)?;
                    println!(
                        "==> [Sandbox] Promoted {} file(s) containing only accepted patches; {} finding(s) remain open (rejected/unconfirmed/failed) and need manual work.", count, failed_fix_count + unconfirmed_count
                    );
                } else if !sandbox_modified_files.is_empty() {
                    println!(
                        "==> [Sandbox] Test suite failed. Authoritative target left UNCHANGED."
                    );
                }
            }

            if generate_report {
                report_md.push_str("\n# Final Audit Summary\n\n");
                report_md.push_str(&format!("- **Actionable findings:** {}\n", actionable_count));
                report_md.push_str(&format!("- **Sink removed (runtime unverified):** {}\n", fixed_count));
                report_md.push_str(&format!("- **Remediation failed:** {}\n", failed_fix_count));
                report_md.push_str(&format!("- **Unconfirmed:** {}\n", unconfirmed_count));
                report_md
                    .push_str(&format!("- **Already resolved:** {}\n", already_resolved_count));
                report_md.push_str(&format!("- **Explicitly safe:** {}\n", safe_count));
                report_md
                    .push_str(&format!("- **Unknown (needs manual review):** {}\n", unknown_count));
                report_md.push_str(&format!(
                    "- **Test Suite Status:** {}\n",
                    if tests_passed { "PASSED / SKIPPED" } else { "FAILED" }
                ));

                let success_rate = if actionable_count > 0 { fixed_count as f64 / actionable_count as f64 } else { 1.0 };
                if remediation_clean && actionable_count > 0 {
                    report_md.push_str("\n## Remediation Integrity\n\n**PASS** — Every actionable finding was verified fixed and passed all gates.\n\n");
                } else if success_rate >= 0.8 && actionable_count > 0 {
                    report_md.push_str(&format!("\n## Remediation Integrity\n\n**PARTIAL** — {:.0}% of actionable findings remediated ({} of {}).\n\n", success_rate * 100.0, fixed_count, actionable_count));
                } else {
                    report_md.push_str(&format!("\n## Remediation Integrity\n\n**FAIL** — {:.0}% remediated ({} of {}).\n\n", success_rate * 100.0, fixed_count, actionable_count));
                }

                let fmt_lower = format.trim().to_ascii_lowercase();
                let summary_json = serde_json::json!({
                    "actionable_findings": actionable_count,
                    "verified_fixed": fixed_count,
                    "remediation_failed": failed_fix_count,
                    "unconfirmed": unconfirmed_count,
                    "already_resolved": already_resolved_count,
                    "explicitly_safe": safe_count,
                    "unknown": unknown_count,
                    "tests_passed": tests_passed,
                    "remediation_clean": remediation_clean
                });

                if let Some(ref sarif_path) = sarif_out {
                    let sarif_str = render_sarif(&audit_records, &rules, &scan_path)?;
                    fs::write(sarif_path, sarif_str)?;
                    println!("\n==> [Report Generator] 📄 SARIF 2.1.0 written to {}", sarif_path.display());
                }
                if let Some(ref j_path) = json_out {
                    let json_str = render_json(&audit_records, summary_json.clone())?;
                    fs::write(j_path, json_str)?;
                    println!("==> [Report Generator] 📄 JSON written to {}", j_path.display());
                }

                match fmt_lower.as_str() {
                    "sarif" => {
                        let out_path = sarif_out.clone().unwrap_or_else(|| PathBuf::from("bounty_report.sarif"));
                        if sarif_out.is_none() {
                            let sarif_str = render_sarif(&audit_records, &rules, &scan_path)?;
                            fs::write(&out_path, sarif_str)?;
                            println!("\n==> [Report Generator] 📄 {} generated.", out_path.display());
                        }
                    }
                    "json" => {
                        let out_path = json_out.clone().unwrap_or_else(|| PathBuf::from("bounty_report.json"));
                        if json_out.is_none() {
                            let json_str = render_json(&audit_records, summary_json)?;
                            fs::write(&out_path, json_str)?;
                            println!("\n==> [Report Generator] 📄 {} generated.", out_path.display());
                        }
                    }
                    _ => {
                        fs::write("bounty_report.md", &report_md)?;
                        println!("\n==> [Report Generator] 📄 bounty_report.md generated.");
                    }
                }
                println!("    Actionable findings : {}", actionable_count);
                println!("    Sink removed (unverified): {}", fixed_count);
                println!("    Remediation failed  : {}", failed_fix_count);
                println!("    Unconfirmed         : {}", unconfirmed_count);
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
                let commit_msg = format!(
                    "chore(security): automated remediation of {} vulnerabilities via AthenaFusionX",
                    fixed_count
                );
                let commit_status = Command::new("git")
                    .args(["commit", "-m", &commit_msg])
                    .current_dir(&scan_path)
                    .status();

                if commit_status.map(|s| s.success()).unwrap_or(false) {
                    println!(
                        "    [🚀 Git Commit OK] Branch `{}` is ready for Pull Request creation.",
                        branch_name
                    );
                }
            }
            if auto_heal && !remediation_clean {
                std::process::exit(2);
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
        Commands::Study { path, output } => {
            nexus_ast_tool::study::run_study(&path, &output)?;
        }
        Commands::Approve { candidate_id, report_identity, confirm, output } => {
            let approval = nexus_ast_tool::evolution::HumanApproval::authorize(
                &candidate_id,
                &report_identity,
                &confirm,
            )?;

            fs::create_dir_all(&output)?;

            let safe_name =
                format!("{}.json", approval.approval_id.replace([':', '|', '=', '/', ' '], "_"));

            let approval_path = output.join(safe_name);
            let serialized = serde_json::to_string_pretty(&approval)?;

            fs::write(&approval_path, serialized)?;

            println!("==> [Human Approval #1] EXPLICITLY AUTHORIZED");
            println!("    Candidate: {}", approval.candidate_id);
            println!("    Report: {}", approval.report_identity);
            println!("    Approval: {}", approval.approval_id);
            println!("    Artifact: {}", approval_path.display());
            println!("    Dry run authorized: YES");
            println!("    Dry run executed: NO");
            println!("    Promotion performed: NO");
        }

        Commands::Tui { dir } => {
            nexus_ast_tool::tui::run(Some(dir))?;
        }
        Commands::Review { hold, dir } => match hold {
            Some(id) => nexus_ast_tool::hold::show(&dir, &id)?,
            None => nexus_ast_tool::hold::list(&dir)?,
        },
        Commands::Promote { hold, dir } => {
            let (ok, conflicts) = nexus_ast_tool::hold::promote(&dir, &hold)?;
            println!("==> [Promote] {} file(s) promoted, {} conflict(s)", ok, conflicts);
            if conflicts > 0 {
                std::process::exit(3);
            }
        }
        Commands::Optimize { path, output } => {
            nexus_ast_tool::supervisor::Supervisor::self_optimize(&path, &output)?;
        }

        Commands::DryRun { report, approval, output } => {
            nexus_ast_tool::supervisor::Supervisor::isolated_dry_run(&report, &approval, &output)?;
        }
    }
    Ok(())
}

/// PoC verdicts block promotion unless NEXUS_POC_MODE=advisory.
fn poc_strict_mode() -> bool {
    std::env::var("NEXUS_POC_MODE").map_or(true, |v| v != "advisory")
}
