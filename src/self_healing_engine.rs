use crate::{
    arbiter_agent::ArbiterAgent,
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

#[derive(Debug, Clone, PartialEq)]
pub enum HealingLayer { Kernel, Backend, Frontend, Swarm, MemoryPressure, OrphanedModules }

pub struct HealingIssue {
    pub layer: HealingLayer,
    pub description: String,
    pub cvss: f32,
}

pub struct SelfHealingEngine {
    state: Arc<AppState>,
    governor: ThermalGovernor,
}

impl SelfHealingEngine {
    pub fn new(state: Arc<AppState>, _memory: Arc<Mutex<MemoryPalace>>) -> Self {
        Self { state, governor: ThermalGovernor::new() }
    }

    pub async fn ignite_autonomic_loop(self) {
        println!("\x1b[36m>> [SELF-HEALING v2.2] Scheduler-Integrated Autonomic Loop Active\x1b[0m");

        loop {
            self.governor.apply_cooldown().await;

            let vitals = {
                let mut monitor = self.state.monitor.lock().await;
                monitor.get_vitals()
            };

            if vitals.cpu_load > 0.80 || vitals.temperature > 68.0 || vitals.ram_gb > vitals.total_ram_gb * 0.82 {
                let _ = self.state.scheduler_tx.send(NexusTask {
                    payload: "THROTTLE_SEMAPHORE".into(),
                    priority: 0,
                }).await;
                println!("\x1b[33m[HEALING] High pressure — scheduler throttled\x1b[0m");
            }

            let issues = self.diagnose().await;
            for issue in issues {
                let arbiter = ArbiterAgent::new(self.state.agent.clone());
                if let Ok(verdict) = arbiter.analyze_directive(&self.state.api_key, &issue.description, &vitals).await {
                    if !verdict.can_proceed { continue; }

                    let mission = self.create_mission(&issue).await;
                    let _ = self.state.scheduler_tx.send(NexusTask {
                        payload: format!("HEAL: {:?}", issue.layer),
                        priority: 2,
                    }).await;

                    let _ = crate::mission_planner::MissionPlanner::execute(&self.state, mission).await;
                }
            }

            sleep(Duration::from_secs(120)).await;
        }
    }

    async fn diagnose(&self) -> Vec<HealingIssue> {
        let mut issues = Vec::new();
        if let Ok(log) = Inquisitor::audit_backend() {
            if log != "NOMINAL" {
                issues.push(HealingIssue { layer: HealingLayer::Backend, description: log, cvss: 7.0 });
            }
        }
        issues
    }

    async fn create_mission(&self, issue: &HealingIssue) -> Mission {
        Mission {
            id: format!("heal-{}", Utc::now().timestamp()),
            goal: format!("Repair {:?}", issue.layer),
            steps: vec![MissionStep { id: 1, description: "Lazarus check".into(), action: None, status: StepStatus::Pending, result: None }],
            created_at: Utc::now().to_rfc3339(),
            completed: false,
        }
    }
}
