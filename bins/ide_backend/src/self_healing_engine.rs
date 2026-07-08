//! SelfHealingEngine — polls `cargo check` on a slow interval and, if errors
//! are found, fires a RepoDoctor directive through the agent pipeline.
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration};
use nexus_substrate::memory::MemoryPalace;
use nexus_runtime_core::shared_state::VmoState;
use nexus_runtime_core::distributed_scheduler::DistributedScheduler;
use nexus_runtime_core::hypothesis_engine::HypothesisEngine;

pub fn init() {
    tracing::info!(">> [SELF_HEALING] Suture engine initialised");
}

pub fn spawn(
    orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>,
    client: reqwest::Client,
    api_key: String,
    memory: Arc<Mutex<MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) {
    tokio::spawn(async move {
        // First run after 5 minutes, then every 20 minutes
        tokio::time::sleep(Duration::from_secs(5 * 60)).await;
        let mut ticker = interval(Duration::from_secs(20 * 60));
        loop {
            ticker.tick().await;
            tracing::info!("[SELF_HEALING] Running cargo check…");

            let output = tokio::process::Command::new("cargo")
                .args(["check", "--message-format=short", "--quiet"])
                .output()
                .await;

            match output {
                Ok(out) if out.status.success() => {
                    tracing::info!("[SELF_HEALING] Substrate integrity verified ✓");
                    let _ = tx.send(
                        serde_json::json!({"type":"LOG_EVENT","payload":"[HEAL] cargo check: PASS"})
                            .to_string()
                    );
                }
                Ok(out) => {
                    let raw_stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                    // Defense in depth alongside the colon-path guard in
                    // executor::sanitize -- strip "file:line:col" diagnostic
                    // locators down to just the file path before this text
                    // ever reaches a directive/prompt, so the Translator LLM
                    // is less likely to try treating a locator as a path in
                    // the first place.
                    let stderr: String = raw_stderr
                        .lines()
                        .map(|line| {
                            if let Some(colon_pos) = line.find(".rs:") {
                                let after = &line[colon_pos + 3..];
                                let mut end = after.len();
                                let mut seen_colons = 0;
                                for (i, c) in after.char_indices() {
                                    if c == ':' {
                                        seen_colons += 1;
                                        if seen_colons == 3 {
                                            end = i;
                                            break;
                                        }
                                    } else if !c.is_ascii_digit() {
                                        end = i; // FIXED: Preserve the rest of the error message!
                                        break;
                                    }
                                }
                                format!("{}{}", &line[..colon_pos + 3], &after[end.min(after.len())..])
                            } else {
                                line.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("
");
                    tracing::warn!("[SELF_HEALING] Errors detected — triggering RepoDoctor");
                    let _ = tx.send(
                        serde_json::json!({"type":"LOG_EVENT",
                            "payload": format!("[HEAL] cargo check FAILED — engaging RepoDoctor")})
                            .to_string()
                    );

                    let mut directive = format!(
                        "diagnose and fix the following cargo check errors:\n{}",
                        &stderr[..stderr.len().min(2000)]
                    );

                    // ⚡ ENGAGE GoT HYPOTHESIS ENGINE
                    if let Ok(got) = HypothesisEngine::evaluate_fault(&client, &api_key, "Cargo check failing", &stderr).await {
                        tracing::info!("[GOT] Root Cause Selected: {}", got.root_cause_analysis);
                        let _ = tx.send(serde_json::json!({"type":"LOG_EVENT", "payload": format!("[GOT] Root Cause: {}", got.root_cause_analysis)}).to_string());
                        directive = format!("{}\n\nGoT RCA: {}\nProposed Fix: {}", directive, got.root_cause_analysis, got.selected_path.proposed_fix_action);
                    }

                    nexus_runtime_core::task_manager::send_task(
                        orchestrator.clone(),
                        nexus_runtime_core::task_manager::TaskContext {
                            client: client.clone(), api_key: api_key.clone(), memory: memory.clone(), vmo_state: vmo_state.clone(), tx: tx.clone(), dist: dist.clone()
                        },
                        &directive
                    ).await;
                }
                Err(e) => tracing::warn!("[SELF_HEALING] Could not run cargo check: {}", e),
            }
        }
    });
}
