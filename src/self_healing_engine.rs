// src/self_healing_engine.rs — v2.3 Gated Autonomic Healing
// FIX: cargo check interval was 120s — on ARM64 this causes two
//      concurrent compilations consuming all 4GB RAM.
// FIX: Added CPU/thermal gate — healing suppressed while hot.
// FIX: Static COMPILE_LOCK prevents concurrent cargo check calls.
use crate::{
    inquisitor::Inquisitor,
    memory::MemoryPalace,
    mission_planner::{Mission, MissionStep, StepStatus},
    scheduler::NexusTask,
    server::AppState,
    thermal_governor::ThermalGovernor,
};
use chrono::Utc;
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;
use tokio::sync::{Mutex, TryAcquireError};
use tokio::time::{sleep, Duration};

/// COMPILE_LOCK: Prevents concurrent cargo check invocations.
static COMPILE_LOCK: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

pub struct SelfHealingEngine {
    state: Arc<AppState>,
    governor: ThermalGovernor,
}

impl SelfHealingEngine {
    pub fn new(state: Arc<AppState>, _memory: Arc<Mutex<MemoryPalace>>) -> Self {
        Self { state, governor: ThermalGovernor::new() }
    }

    pub async fn ignite_autonomic_loop(self) {
        println!("\x1b[36m>> [SELF-HEALING v2.3] Autonomic Immune System Active\x1b[0m");

        loop {
            // FIX: Bumped from 120s to 600s to avoid concurrent compilation
            sleep(Duration::from_secs(600)).await;

            self.governor.apply_cooldown().await;

            let vitals = self.state.monitor.lock().await.get_vitals();

            // Substrate protection — throttle scheduler if stressed
            if vitals.temperature > 65.0 || vitals.ram_gb > (vitals.total_ram_gb * 0.85) {
                let _ = self
                    .state
                    .scheduler_tx
                    .send(NexusTask { payload: "THROTTLE_SEMAPHORE".into(), priority: 0 })
                    .await;
                println!("\x1b[33m[HEALING] System stress. Throttling scheduler.\x1b[0m");
                continue; // Skip cargo check when already under pressure
            }

            // FIX: Gate cargo check behind CPU threshold
            if vitals.cpu_load > 0.5 {
                println!(
                    "\x1b[33m[HEALING] CPU at {:.0}% — skipping cargo check.\x1b[0m",
                    vitals.cpu_load * 100.0
                );
                continue;
            }

            // FIX: Non-blocking try_acquire — skip if already compiling
            match COMPILE_LOCK.clone().try_acquire_owned() {
                Err(TryAcquireError::NoPermits) => {
                    println!("[HEALING] Compilation already in progress — skipping.");
                    continue;
                }
                Err(e) => {
                    eprintln!("[HEALING] Lock error: {}", e);
                    continue;
                }
                Ok(_guard) => {
                    if let Ok(log) = Inquisitor::audit_backend() {
                        if log != "NOMINAL" {
                            println!("\x1b[31m[HEALING] Backend fracture detected.\x1b[0m");
                            let mission = Mission {
                                id: format!("heal-{}", Utc::now().timestamp()),
                                goal: "Repair Backend Fracture".into(),
                                steps: vec![MissionStep {
                                    id: 1,
                                    description: "Lazarus Restore Check".into(),
                                    action: None,
                                    status: StepStatus::Pending,
                                    result: None,
                                }],
                                created_at: Utc::now().to_rfc3339(),
                                completed: false,
                            };
                            let _ = crate::mission_planner::MissionPlanner::execute(
                                &self.state,
                                mission,
                            )
                            .await;
                        } else {
                            println!("[HEALING] Backend nominal.");
                        }
                    }
                    // _guard dropped here — releases COMPILE_LOCK
                }
            }
        }
    }
}
