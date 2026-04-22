use crate::{
    inquisitor::Inquisitor,
    memory::MemoryPalace,
    mission_planner::{Mission, MissionStep, StepStatus},
    scheduler::NexusTask,
    server::AppState,
    thermal_governor::ThermalGovernor,
};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub struct SelfHealingEngine {
    state: Arc<AppState>,
    governor: ThermalGovernor,
}

impl SelfHealingEngine {
    pub fn new(state: Arc<AppState>, _memory: Arc<Mutex<MemoryPalace>>) -> Self {
        Self { state, governor: ThermalGovernor::new() }
    }

    pub async fn ignite_autonomic_loop(self) {
        println!("\x1b[36m>> [SELF-HEALING v2.2] Autonomic Immune System Active\x1b[0m");

        loop {
            self.governor.apply_cooldown().await;

            let vitals = self.state.monitor.lock().await.get_vitals();

            // ⚡ SUBSTRATE PROTECTION: Choke the scheduler if feverish or OOM
            if vitals.temperature > 65.0 || vitals.ram_gb > (vitals.total_ram_gb * 0.85) {
                let _ = self
                    .state
                    .scheduler_tx
                    .send(NexusTask { payload: "THROTTLE_SEMAPHORE".into(), priority: 0 })
                    .await;
                println!("\x1b[33m[HEALING] System Stress Detected. Throttling Scheduler.\x1b[0m");
            }

            // DIAGNOSE BACKEND
            if let Ok(log) = Inquisitor::audit_backend() {
                if log != "NOMINAL" {
                    println!(
                        "\x1b[31m[HEALING] Backend Fracture Detected. Initiating Repair.\x1b[0m"
                    );
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
                    let _ =
                        crate::mission_planner::MissionPlanner::execute(&self.state, mission).await;
                }
            }

            sleep(Duration::from_secs(120)).await;
        }
    }
}
