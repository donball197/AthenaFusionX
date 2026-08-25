use anyhow::Result;
use std::process::ExitStatus;
use std::os::unix::process::ExitStatusExt;

pub struct CompilerCritic;

pub struct CriticOutcome {
    pub success: bool,
    pub evidence: String,
    pub auto_heals_applied: usize,
}

impl CompilerCritic {
    pub async fn verify_mutations(mutation_paths: &[String]) -> Result<Option<CriticOutcome>> {
        let is_rust = mutation_paths.iter().any(|p| p.ends_with(".rs"));
        let is_python = mutation_paths.iter().any(|p| p.ends_with(".py"));
        let is_js = mutation_paths.iter().any(|p| p.ends_with(".js") || p.ends_with(".ts"));
        let needs_verification = is_rust || is_python || is_js;

        if !needs_verification {
            return Ok(None);
        }

        let compile_output = if is_rust {
            tokio::process::Command::new("cargo")
                .arg("check")
                .arg("--message-format=json")
                .output()
                .await
        } else if is_python {
            let mut all_ok = true;
            let mut combined_stderr = String::new();
            for p in mutation_paths {
                if p.ends_with(".py") {
                    match tokio::process::Command::new("python3").arg("-m").arg("py_compile").arg(p).output().await {
                        Ok(o) if !o.status.success() => {
                            all_ok = false;
                            combined_stderr.push_str(&format!("{}: {}\n", p, String::from_utf8_lossy(&o.stderr)));
                        }
                        Ok(_) => {}
                        Err(e) => { all_ok = false; combined_stderr.push_str(&format!("{}: python3 not found: {}\n", p, e)); }
                    }
                }
            }
            Ok(std::process::Output { status: ExitStatus::from_raw(if all_ok { 0 } else { 1 }), stdout: Vec::new(), stderr: combined_stderr.into_bytes() })
        } else {
            let mut all_ok = true;
            let mut combined_stderr = String::new();
            for p in mutation_paths {
                if p.ends_with(".js") || p.ends_with(".ts") {
                    match tokio::process::Command::new("node").arg("--check").arg(p).output().await {
                        Ok(o) if !o.status.success() => {
                            all_ok = false;
                            combined_stderr.push_str(&format!("{}: {}\n", p, String::from_utf8_lossy(&o.stderr)));
                        }
                        Ok(_) => {}
                        Err(e) => { all_ok = false; combined_stderr.push_str(&format!("{}: node not found: {}\n", p, e)); }
                    }
                }
            }
            Ok(std::process::Output { status: ExitStatus::from_raw(if all_ok { 0 } else { 1 }), stdout: Vec::new(), stderr: combined_stderr.into_bytes() })
        };

        match compile_output {
            Ok(output) if output.status.success() => {
                let verifier_name = if is_rust { "cargo check" } else if is_python { "py_compile" } else { "node --check" };
                Ok(Some(CriticOutcome { success: true, evidence: format!("\n[VERIFY_OK] {} passed.", verifier_name), auto_heals_applied: 0 }))
            }
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let mut applied_heals = 0;

                if is_rust {
                    for line in stdout.lines() {
                        if let Some(diag) = nexus_ast_tool::engine::parse_compiler_line(line) {
                            if let Some(action) = nexus_ast_tool::engine::determine_policy(&diag) {
                                let file_path = std::path::PathBuf::from(&action.file);
                                if file_path.exists() {
                                    if let Ok(source) = std::fs::read_to_string(&file_path) {
                                        if let Some(strategy) = nexus_ast_tool::mutation::get_strategy(&action.strategy) {
                                            if let Ok(new_source) = strategy.apply(&source, &action.symbol, &action.payload) {
                                                let _ = std::fs::write(&file_path, new_source);
                                                applied_heals += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let err_preview = stderr.lines().take(15).collect::<Vec<_>>().join("\n");
                let fail_evidence = format!("[COMPILE_FAIL] Your patch caused a compilation error:\n{}", err_preview);

                Ok(Some(CriticOutcome { success: false, evidence: fail_evidence, auto_heals_applied: applied_heals }))
            }
            Err(e) => Ok(Some(CriticOutcome { success: false, evidence: format!("[COMPILER FAULT] failed to run validator: {}", e), auto_heals_applied: 0 }))
        }
    }
}
