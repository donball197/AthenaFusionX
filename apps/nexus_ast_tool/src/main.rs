use anyhow::Result;
use clap::{Parser, Subcommand};
use nexus_ast_tool::polyglot::{PolyglotScanner, SecurityFinding};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "nexus_ast_tool", version = "0.1.0", about = "Nexus Singularity Polyglot AST Engine")]
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

/// Secondary syntax check via external linters, run against a TEMP sibling
/// file so the real target is never touched before verification passes.
/// None       => no linter available (tree-sitter gate remains primary);
/// Some(false) => the linter rejected the patched source.
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
        Err(_) => None, // linter binary not installed; tree-sitter gate remains
    }
}

/// Resolve the splice anchor (0-based inclusive line indices).
///
/// Primary: the recorded sink lines must still hold the exact snippet.
/// Fallback: locate the snippet by text, disambiguated by proximity to the
/// recorded sink line; ties are REJECTED rather than guessed.
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
    let best: Vec<usize> = candidates
        .iter()
        .filter(|(d, _)| *d == min_dist)
        .map(|(_, i)| *i)
        .collect();
    match best.as_slice() {
        [only] => Some((*only, *only + snippet_lines - 1)),
        _ => None, // zero matches or ambiguous — refuse to guess
    }
}

/// Extract the query argument from a call like `mysqli_query($con, $q)`,
/// `mysqli_multi_query($con, $q)`, or `$con->query($q)`. Uses bracket-depth
/// matching from the call marker (not sink_text's outer parens, which may
/// belong to a wrapping `if (...)`). Returns None (refuse to guess) unless
/// the LAST argument is a single bare `$variable`.
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
                let status = Command::new("git")
                    .args(["clone", "--depth", "1", &repo_url, scan_path.to_str().unwrap()])
                    .status()?;
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

            // Every actionable finding ends in exactly one of these buckets:
            let mut actionable_count = 0;
            let mut fixed_count = 0;
            let mut failed_fix_count = 0;
            let mut already_resolved_count = 0;
            let mut safe_count = 0;
            let mut unknown_count = 0;
            let mut triage_error_count = 0;
            let mut unknown_entries: Vec<String> = Vec::new();
            let mut taint_deferred_count = 0;
            let mut taint_entries: Vec<String> = Vec::new();

            if ai_triage {
                println!("\n==> [AI Triage] Initializing LLM to filter false positives...");
                // Descending sink order within each file: patches made at higher
                // lines never shift the stored anchors of lower findings.
                findings.sort_by(|a, b| {
                    a.path.cmp(&b.path).then(b.sink_start_line.cmp(&a.sink_start_line))
                });

                for finding in findings {
                    println!("\n    [?] Evaluating {} at line {}...",
                        finding.pattern_name.to_uppercase(), finding.sink_start_line);
                    let file_path_str = finding.path.display().to_string();
                    let lang_str = format!("{:?}", finding.language);

                    match nexus_ast_tool::client::ApiClient::triage_vulnerability(
                        &finding.pattern_name, &finding.source_snippet,
                    ) {
                        Ok((state_raw, llm_severity, reason)) => {
                            let classification = match state_raw.trim().to_uppercase().as_str() {
                                "EXPLOITABLE" => "EXPLOITABLE",
                                "SAFE" => "SAFE",
                                other => {
                                    if other != "UNKNOWN" {
                                        println!("        [⚠️ TRIAGE POLICY] Invalid model state '{}'; forcing UNKNOWN.", other);
                                    }
                                    "UNKNOWN"
                                }
                            };

                            if classification == "EXPLOITABLE" {
                                actionable_count += 1;
                                println!("        [🔴 EXPLOITABLE] Rule severity: {} (model said: {})", finding.severity, llm_severity);
                                println!("        [🧠 AI Reasoning] {}", reason);

                                report_md.push_str(&format!("## 🚨 Vulnerability: {}\n", finding.pattern_name.to_uppercase()));
                                report_md.push_str(&format!("- **Severity:** {}\n", finding.severity));
                                report_md.push_str(&format!("- **File:** `{}` (Sink lines {}-{})\n\n",
                                    file_path_str, finding.sink_start_line, finding.sink_end_line));
                                report_md.push_str(&format!("### Description\n{}\n\n", reason));
                                report_md.push_str(&format!("### Vulnerable Code\n```{}\n{}\n```\n\n",
                                    lang_str.to_lowercase(), finding.patch_snippet));

                                if auto_heal {
                                    println!("        [🛠️ Auto-Heal] Generating secure anchor-based patch...");
                                    // NOTE: every path below ends in `continue` with its
                                    // own report entry, preserving the invariant
                                    // actionable == fixed + failed + already_resolved.

                                    // (1) Fail-closed read.
                                    let file_source = match fs::read_to_string(&finding.path) {
                                        Ok(src) => src,
                                        Err(e) => {
                                            println!("        [❌ Patch Error] Fail-closed read failure: {}", e);
                                            failed_fix_count += 1;
                                            report_md.push_str(&format!(
                                                "### Patch (FAILED: I/O read error: {})\n\n---\n\n", e));
                                            continue;
                                        }
                                    };

                                    // (1.5) TAINT GATE: refuse to auto-heal if the query
                                    // variable was built via string concatenation elsewhere
                                    // (multi-site taint) rather than at the sink itself. A
                                    // single-site splice here would only rewrite the execution
                                    // call, leaving the actual injection untouched -- the
                                    // confirmed false "Verified" failure mode from this
                                    // session's live testing (login2.php et al).
                                    if let Some(var_name) = extract_query_var(&finding.sink_text) {
                                        if let Some((c_start, c_end, _)) = nexus_ast_tool::polyglot::find_concat_assignment(
                                            &file_source,
                                            &finding.language.tree_sitter_language(),
                                            &var_name,
                                            finding.sink_start_line,
                                        ) {
                                            taint_deferred_count += 1;
                                            println!(
                                                "        [\u{26a0}\u{fe0f} Multi-Site Taint] {} is built via concatenation at line {}-{}, not at the sink. Cannot safely auto-patch in one splice -- needs manual review.",
                                                var_name, c_start, c_end
                                            );
                                            report_md.push_str(&format!(
                                                "### Patch (DEFERRED: multi-site taint -- `{}` built via concatenation at lines {}-{}, not patched)\n\n---\n\n",
                                                var_name, c_start, c_end
                                            ));
                                            taint_entries.push(format!(
                                                "- `{}` sink lines {}-{} ({}): query variable `{}` built via concatenation at lines {}-{}; auto-heal deferred, requires manual multi-site fix",
                                                file_path_str, finding.sink_start_line, finding.sink_end_line,
                                                finding.pattern_name, var_name, c_start, c_end
                                            ));
                                            continue;
                                        }
                                    }

                                    // (2) SEARCH-as-assertion: skip sites already fixed by
                                    // an earlier patch in this run, BEFORE spending an LLM call.
                                    if !file_source.contains(&finding.sink_text) {
                                        println!("        [ℹ️ Already Resolved] Sink text no longer present. Skipping.");
                                        already_resolved_count += 1;
                                        report_md.push_str(
                                            "### Patch (skipped: site already resolved by an earlier patch in this run)\n\n---\n\n");
                                        continue;
                                    }

                                    // (3) Anchor resolution: stored lines must still hold
                                    // the exact sink span; otherwise relocate or refuse.
                                    let Some((start_idx, end_idx)) = resolve_anchor(&file_source, &finding) else {
                                        println!("        [❌ Patch Rejected] Anchor drift: recorded sink lines no longer match the file.");
                                        failed_fix_count += 1;
                                        report_md.push_str(
                                            "### Patch (REJECTED: anchor drift - recorded sink lines no longer match the file)\n\n---\n\n");
                                        continue;
                                    };

                                    // (4) Model payload: replacement ONLY, no SEARCH block.
                                    let replacement = match nexus_ast_tool::client::ApiClient::generate_security_patch(
                                        &finding.pattern_name, &finding.patch_snippet, &lang_str,
                                    ) {
                                        Ok(r) => r,
                                        Err(e) => {
                                            println!("        [❌ Patch Gen Error] {}", e);
                                            failed_fix_count += 1;
                                            report_md.push_str(&format!(
                                                "### Patch (FAILED: model error: {})\n\n---\n\n", e));
                                            continue;
                                        }
                                    };
                                    if replacement.trim().is_empty() {
                                        println!("        [❌ Patch Rejected] Model returned an empty replacement.");
                                        failed_fix_count += 1;
                                        report_md.push_str("### Patch (REJECTED: empty replacement)\n\n---\n\n");
                                        continue;
                                    }
                                    let cleaned_patch = replacement.trim().to_string();

                                    // (5) Splice IN MEMORY, preserving the file's line endings.
                                    let lines: Vec<&str> = file_source.lines().collect();
                                    let crlf = file_source.matches("\r\n").count() * 2
                                        > file_source.matches('\n').count();
                                    let sep = if crlf { "\r\n" } else { "\n" };
                                    let patch_text = if crlf {
                                        cleaned_patch.replace('\n', "\r\n")
                                    } else {
                                        cleaned_patch.clone()
                                    };

                                    let mut new_source = String::with_capacity(file_source.len() + patch_text.len());
                                    new_source.push_str(&lines[..start_idx].join(sep));
                                    if start_idx > 0 {
                                        new_source.push_str(sep);
                                    }
                                    new_source.push_str(&patch_text);
                                    if end_idx + 1 < lines.len() {
                                        new_source.push_str(sep);
                                        new_source.push_str(&lines[end_idx + 1..].join(sep));
                                    }
                                    if file_source.ends_with('\n') && !new_source.ends_with('\n') {
                                        new_source.push_str(sep);
                                    }

                                    // (6) GATE 1 — syntax. Primary: in-memory tree-sitter
                                    // parse (fail-closed, all languages). Secondary:
                                    // external linter on a temp sibling file.
                                    let ts_ok = finding.language.parses_cleanly(&new_source);
                                    let lint = if ts_ok {
                                        external_lint(&lang_str, &new_source, &finding.path)
                                    } else {
                                        None
                                    };
                                    if ts_ok && lint.is_none() {
                                        println!("        [ℹ️ Lint] No external linter for {}; tree-sitter gate is primary.", lang_str);
                                    }
                                    let syntax_ok = ts_ok && lint.unwrap_or(true);

                                    // (7) GATE 3 — strict sink elimination: no same-rule
                                    // capture inside the replacement span (±2 drift).
                                    let ast_ok = if syntax_ok {
                                        let new_start = start_idx + 1;
                                        let new_end = new_start + patch_text.lines().count().saturating_sub(1);
                                        let residuals = PolyglotScanner::scan_source(
                                            &finding.path, &new_source, &rules,
                                        );
                                        !residuals.iter().any(|f| {
                                            f.pattern_name == finding.pattern_name
                                                && f.sink_start_line + 2 >= new_start
                                                && f.sink_end_line <= new_end + 2
                                        })
                                    } else {
                                        false
                                    };

                                    if syntax_ok && ast_ok {
                                        // The ONLY disk write happens after all gates pass.
                                        // There is no write-then-revert path to fail.
                                        if let Err(e) = fs::write(&finding.path, &new_source) {
                                            println!("        [❌ Patch Error] Failed to write verified patch: {}", e);
                                            failed_fix_count += 1;
                                            report_md.push_str(&format!(
                                                "### Patch (FAILED: I/O write error: {})\n\n---\n\n", e));
                                            continue;
                                        }
                                        fixed_count += 1;
                                        println!("        [✅ Patch Verified] Syntax OK and strict AST rescan confirms the sink is eliminated.");
                                        report_md.push_str("### Recommended Patch (Auto-Healed, Verified)\n");
                                        report_md.push_str(&format!("```{}\n{}\n```\n\n---\n\n",
                                            lang_str.to_lowercase(), cleaned_patch));
                                        continue;
                                    }

                                    let reject_reason = if !ts_ok {
                                        "tree-sitter syntax gate failed"
                                    } else if lint == Some(false) {
                                        "external linter rejected the patch"
                                    } else {
                                        "AST rescan still detects the sink (escape-only patch?)"
                                    };
                                    failed_fix_count += 1;
                                    println!("        [❌ Patch Rejected] {}. Nothing was written.", reject_reason);
                                    report_md.push_str(&format!(
                                        "### Recommended Patch (REJECTED: {} - never written)\n```{}\n{}\n```\n\n---\n\n",
                                        reject_reason, lang_str.to_lowercase(), cleaned_patch));
                                    continue;
                                }

                                // No auto-heal requested: close this report section.
                                report_md.push_str("---\n\n");
                            } else if classification == "SAFE" {
                                safe_count += 1;
                                println!("        [🟢 SAFE] Explicitly classified safe.");
                                println!("        [🧠 AI Reasoning] {}", reason);
                            } else {
                                unknown_count += 1;
                                println!("        [🟡 UNKNOWN] Security state cannot be established without assumptions.");
                                println!("        [🧠 AI Reasoning] {}", reason);
                                unknown_entries.push(format!(
                                    "- `{}` sink lines {}-{} ({}): {}",
                                    file_path_str, finding.sink_start_line, finding.sink_end_line,
                                    finding.pattern_name, reason
                                ));
                            }
                        }
                        Err(e) => {
                            triage_error_count += 1;
                            println!("        [⚠️ API Error] {}", e);
                        }
                    }
                }

                if generate_report {
                    report_md.push_str("\n# Final Audit Summary\n\n");
                    report_md.push_str(&format!("- **Actionable findings:** {}\n", actionable_count));
                    report_md.push_str(&format!("- **Verified fixed:** {}\n", fixed_count));
                    report_md.push_str(&format!("- **Remediation failed:** {}\n", failed_fix_count));
                    report_md.push_str(&format!("- **Already resolved (earlier patch in this run):** {}\n", already_resolved_count));
                    report_md.push_str(&format!("- **Explicitly safe:** {}\n", safe_count));
                    report_md.push_str(&format!("- **Unknown (needs manual review):** {}\n", unknown_count));
                    report_md.push_str(&format!("- **Deferred (multi-site taint, not patched):** {}\n", taint_deferred_count));
                    report_md.push_str(&format!("- **Triage errors (API):** {}\n\n", triage_error_count));

                    if triage_error_count > 0 {
                        report_md.push_str(&format!(
                            "> ⚠️ {} finding(s) could not be triaged after retries and are not included above. Re-run recommended.\n\n",
                            triage_error_count
                        ));
                    }
                    if !unknown_entries.is_empty() {
                        report_md.push_str("## Needs Manual Review\n\n");
                        for entry in &unknown_entries {
                            report_md.push_str(entry);
                            report_md.push('\n');
                        }
                        report_md.push('\n');
                    }

                    if !taint_entries.is_empty() {
                        report_md.push_str("## Deferred: Multi-Site Taint (auto-heal not attempted)\n\n");
                        for entry in &taint_entries {
                            report_md.push_str(entry);
                            report_md.push('\n');
                        }
                        report_md.push('\n');
                    }

                    // Integrity: every actionable finding must be verified fixed
                    // OR resolved by an earlier patch in this run, zero failures.
                    let remediation_clean = failed_fix_count == 0
                        && taint_deferred_count == 0
                        && actionable_count == fixed_count + already_resolved_count + taint_deferred_count;

                    if remediation_clean {
                        report_md.push_str("## Remediation Integrity\n\n**PASS** — Every actionable finding was verified fixed or resolved.\n\n");
                    } else {
                        report_md.push_str("## Remediation Integrity\n\n**FAIL** — One or more actionable findings were not verified fixed.\n\n");
                    }

                    fs::write("bounty_report.md", &report_md)?;

                    println!("\n==> [Report Generator] 📄 bounty_report.md generated.");
                    println!("    Actionable findings : {}", actionable_count);
                    println!("    Verified fixed      : {}", fixed_count);
                    println!("    Remediation failed  : {}", failed_fix_count);
                    println!("    Already resolved    : {}", already_resolved_count);
                    println!("    Explicitly safe     : {}", safe_count);
                    println!("    Unknown             : {}", unknown_count);
                    println!("    Triage errors       : {}", triage_error_count);
                    if remediation_clean {
                        println!("    [✅ REMEDIATION INTEGRITY] PASS");
                    } else {
                        println!("    [❌ REMEDIATION INTEGRITY] FAIL");
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
