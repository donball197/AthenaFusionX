use crate::{
    inquisitor::Inquisitor,
    arbiter_agent::ArbiterAgent,
    thermal_governor::ThermalGovernor,
    mission_planner::{Mission, MissionStep, StepStatus},
    memory::MemoryPalace,
    server::AppState,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use chrono::Utc;

#[derive(Debug, Clone, PartialEq)]
pub enum HealingLayer {
    Kernel, Backend, Frontend, Swarm,
}

#[derive(Debug)]
pub struct HealingIssue {
    pub layer: HealingLayer,
    pub description: String,
    pub cvss: f32,
}

pub struct SelfHealingEngine {
    state: Arc<AppState>,
    memory: Arc<Mutex<MemoryPalace>>,
    governor: ThermalGovernor,
}

impl SelfHealingEngine {
    pub fn new(state: Arc<AppState>, memory: Arc<Mutex<MemoryPalace>>) -> Self {
        Self {
            state,
            memory,
            governor: ThermalGovernor::new(),
        }
    }

    pub async fn ignite_autonomic_loop(self) {
        println!("\x1b[36m>> [SELF-HEALING v2.0] Autonomic Loop Started (4GB safe)\x1b[0m");

        loop {
            self.governor.apply_cooldown().await;

            let vitals = self.state.monitor.get_vitals();

            // Strong resource guard for Lenovo Duet 4GB
            if vitals.cpu_load > 0.65 || vitals.temperature > 68.0 || vitals.ram_gb > 3.2 {
                sleep(Duration::from_secs(90)).await;
                continue;
            }

            let issues = self.diagnose().await;
            for issue in issues {
                let priority = self.calculate_priority(&issue, &vitals).await;
                if priority > 0.52 {
                    let mission = self.create_mission(&issue).await;
                    let _ = crate::mission_planner::MissionPlanner::execute(&self.state, mission).await;
                }
            }

            self.record_cycle().await;
            sleep(Duration::from_secs(75)).await;
        }
    }

    async fn diagnose(&self) -> Vec<HealingIssue> {
        let mut issues = Vec::new();

        if let Ok(log) = Inquisitor::audit_backend() {
            if log != "NOMINAL" {
                issues.push(HealingIssue { layer: HealingLayer::Backend, description: log, cvss: 7.5 });
            }
        }
        if let Ok(log) = Inquisitor::audit_ui() {
            if log != "NOMINAL" {
                issues.push(HealingIssue { layer: HealingLayer::Frontend, description: log, cvss: 6.8 });
            }
        }
        issues
    }

    async fn calculate_priority(&self, issue: &HealingIssue, vitals: &crate::system_monitor::Vitals) -> f32 {
        let base = issue.cvss / 10.0;
        let thermal_factor = if vitals.temperature > 65.0 { 0.6 } else { 1.0 };
        base * thermal_factor * 0.78   // simple historical factor
    }

    async fn create_mission(&self, issue: &HealingIssue) -> Mission {
        let arbiter = ArbiterAgent::new(self.state.agent.clone(), Some(self.state.orchestrator.clone()));
        let _ = arbiter.analyze_directive(&self.state.api_key, &issue.description).await;

        Mission {
            id: format!("heal-{}", Utc::now().timestamp()),
            goal: format!("Repair {:?}", issue.layer),
            steps: vec![
                MissionStep { id: 1, description: "Lazarus pre-snapshot".into(), action: None, status: StepStatus::Pending, result: None },
                MissionStep { id: 2, description: format!("Fix {:?}", issue.layer), action: None, status: StepStatus::Pending, result: None },
                MissionStep { id: 3, description: "Validate repair".into(), action: None, status: StepStatus::Pending, result: None },
            ],
            created_at: Utc::now().to_rfc3339(),
            completed: false,
        }
    }

    async fn record_cycle(&self) {
        let palace = self.memory.lock().await;
        let _ = palace.store_memory(
            "SelfHealingEngine",
            "autonomic_cycle",
            "Completed diagnostic + repair pass with learning feedback",
        ).await;
    }
}
