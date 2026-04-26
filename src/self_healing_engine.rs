// src/self_healing_engine.rs — v3.4 [SNAPSHOT_FILE_FIX + FIRST_CYCLE_FAST]
use crate::{
    inquisitor::Inquisitor,
    lazarus_vault::LazarusVault,
    memory::MemoryPalace,
    neural_agent::ContextPacket,
    server::AppState,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Semaphore, Mutex};
use tokio::time::{sleep, Duration};

static COMPILE_LOCK: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

// Files to snapshot before a suture attempt (directories excluded — vault rejects them)
const SNAPSHOT_TARGETS: &[&str] = &[
    "src/main.rs",
    "src/lib.rs",
    "src/executor.rs",
    "src/task_manager.rs",
];

pub struct SelfHealingEngine {
    state: Arc<AppState>,
    consecutive_failures: u8,
}

impl SelfHealingEngine {
    pub fn new(state: Arc<AppState>, _memory: Arc<Mutex<MemoryPalace>>) -> Self {
        Self { state, consecutive_failures: 0 }
    }

    pub async fn ignite_autonomic_loop(mut self) {
        println!("\x1b[36m>> [SELF-HEALING v3.4] Neural Suture & Immune Rollback Active\x1b[0m");

        // First check happens after 60s (not 300s) so startup issues are caught quickly.
        let mut interval_secs: u64 = 60;

        loop {
            sleep(Duration::from_secs(interval_secs)).await;
            // Subsequent cycles are every 5 minutes
            interval_secs = 300;

            let vitals = self.state.monitor.lock().await.get_vitals();

            // Thermal safety gate
            if vitals.temperature > 70.0 || vitals.cpu_load > 0.8 {
                println!(
                    "\x1b[33m[SELF-HEALING] Skipping cycle — thermal/load gate (temp={:.1} load={:.2})\x1b[0m",
                    vitals.temperature, vitals.cpu_load
                );
                continue;
            }

            if let Ok(_guard) = COMPILE_LOCK.clone().try_acquire_owned() {
                match Inquisitor::audit_backend().await {
                    Err(e) => {
                        eprintln!("[SELF-HEALING] Inquisitor fault: {}", e);
                    }
                    Ok(log) if log == "NOMINAL" => {
                        self.consecutive_failures = 0;
                    }
                    Ok(log) => {
                        self.consecutive_failures += 1;
                        println!(
                            "\x1b[31m[HEALING] Fracture detected (failure #{}).\x1b[0m",
                            self.consecutive_failures
                        );

                        if self.consecutive_failures >= 3 {
                            println!(
                                "\x1b[31m[IMMUNE_SYSTEM] {} consecutive failures — initiating rollback.\x1b[0m",
                                self.consecutive_failures
                            );
                            let snapshots = LazarusVault::list_snapshots();
                            if let Some(last_stable) = snapshots.last() {
                                println!(
                                    "\x1b[33m[ROLLBACK] Restoring snapshot {}\x1b[0m",
                                    last_stable.hash
                                );
                                let _ = crate::task_manager::send_task(
                                    self.state.orchestrator.clone(),
                                    &self.state.client,
                                    &self.state.api_key,
                                    &format!("restore snapshot {}", last_stable.hash),
                                    self.state.memory.clone(),
                                    self.state.vmo_state.clone(),
                                    self.state.tx.clone(),
                                    self.state.dist.clone(),
                                )
                                .await;
                            } else {
                                println!("[ROLLBACK] No snapshots available.");
                            }
                            self.consecutive_failures = 0;
                        } else {
                            // Snapshot individual files (not the directory)
                            for target in SNAPSHOT_TARGETS {
                                if std::path::Path::new(target).exists() {
                                    let _ =
                                        LazarusVault::auto_snapshot(target, "pre_suture").await;
                                }
                            }
                            self.attempt_neural_suture(&log).await;
                        }
                    }
                }
            }
        }
    }

    async fn attempt_neural_suture(&self, error_log: &str) {
        if error_log.contains("JSON_DECODE_FRACTURE") || error_log.contains("malformed JSON") {
            println!("\x1b[33m[HEALING] JSON framing error detected. Skipping suture to prevent recursive loop.\x1b[0m");
            return;
        }
        let prompt = format!(
            "CONTEXT: Rust/ARM64 Build Failure\nERROR_LOG: {}\nTASK: Identify the file and provide a fix in <SUTURE> tags.",
            error_log
        );

        let ctx = ContextPacket::default();
        if let Ok((response, _)) = self
            .state
            .agent
            .prepare_and_run(
                &self.state.client,
                &self.state.api_key,
                &prompt,
                ctx,
                0.2,
                Some("gemma-4-31b-it"),
            )
            .await
        {
            if response.contains("<SUTURE>") {
                println!(
                    "\x1b[32m[HEALING] Suture payload received. Substrate repair logic primed.\x1b[0m"
                );
            } else {
                println!("[HEALING] No <SUTURE> tag in response. Skipping patch.");
            }
        }
    }
}
