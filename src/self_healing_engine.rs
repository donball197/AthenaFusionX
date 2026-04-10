/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/self_healing_engine.rs - v2.1 (Arbiter result now gates healing)
use crate::{
    arbiter_agent::ArbiterAgent,
    inquisitor::Inquisitor,
    memory::MemoryPalace,
    mission_planner::{Mission, MissionStep, StepStatus},
    server::AppState,
    thermal_governor::ThermalGovernor,
};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, PartialEq)]
pub enum HealingLayer {
    Kernel,
    Backend,
    Frontend,
    Swarm,
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
        Self { state, memory, governor: ThermalGovernor::new() }
    }

    pub async fn ignite_autonomic_loop(self) {
        println!("\x1b[36m>> [SELF-HEALING v2.1] Autonomic Loop Started (4GB safe)\x1b[0m");

        loop {
            self.governor.apply_cooldown().await;

            let vitals = self.state.monitor.get_vitals();

            // Resource guard for Lenovo Duet 4GB
            if vitals.cpu_load > 0.65 || vitals.temperature > 68.0 || vitals.ram_gb > 3.2 {
                sleep(Duration::from_secs(90)).await;
                continue;
            }

            let issues = self.diagnose().await;

            for issue in issues {
                let priority = self.calculate_priority(&issue, &vitals).await;

                if priority <= 0.52 {
                    println!(
                        ">> [SELF-HEALING] Issue priority {:.2} below threshold — skipping.",
                        priority
                    );
                    continue;
                }

                // FIX: Actually use the Arbiter result to decide whether to heal
                let arbiter = ArbiterAgent::new(
                    self.state.agent.clone(),
                    Some(self.state.orchestrator.clone()),
                );

                match arbiter.analyze_directive(&self.state.api_key, &issue.description).await {
                    Ok(report) => {
                        if !report.can_proceed {
                            println!(
                                "\x1b[33m>> [ARBITER] Healing blocked (score {:.2}): {}\x1b[0m",
                                report.risk_score, report.recommendation
                            );
                            continue;
                        }
                        println!(
                            "\x1b[32m>> [ARBITER] Authorized (score {:.2}): {}\x1b[0m",
                            report.risk_score, report.recommendation
                        );
                    }
                    Err(e) => {
                        println!(
                            "\x1b[33m>> [ARBITER] Analysis failed ({}), using fallback gate.\x1b[0m",
                            e
                        );
                        // Fallback: only proceed if CVSS is below critical
                        if issue.cvss >= 9.0 {
                            println!(">> [ARBITER] CVSS >= 9.0 — blocking without Arbiter confirmation.");
                            continue;
                        }
                    }
                }

                let mission = self.create_mission(&issue).await;
                let _ = crate::mission_planner::MissionPlanner::execute(&self.state, mission).await;
            }

            self.record_cycle().await;
            sleep(Duration::from_secs(75)).await;
        }
    }

    async fn diagnose(&self) -> Vec<HealingIssue> {
        let mut issues = Vec::new();

        if let Ok(log) = Inquisitor::audit_backend() {
            if log != "NOMINAL" {
                issues.push(HealingIssue {
                    layer: HealingLayer::Backend,
                    description: log,
                    cvss: 7.5,
                });
            }
        }
        if let Ok(log) = Inquisitor::audit_ui() {
            if log != "NOMINAL" {
                issues.push(HealingIssue {
                    layer: HealingLayer::Frontend,
                    description: log,
                    cvss: 6.8,
                });
            }
        }
        issues
    }

    async fn calculate_priority(
        &self,
        issue: &HealingIssue,
        vitals: &crate::system_monitor::Vitals,
    ) -> f32 {
        let base = issue.cvss / 10.0;
        let thermal_factor = if vitals.temperature > 65.0 { 0.6 } else { 1.0 };
        base * thermal_factor * 0.78
    }

    async fn create_mission(&self, issue: &HealingIssue) -> Mission {
        Mission {
            id: format!("heal-{}", Utc::now().timestamp()),
            goal: format!("Repair {:?}", issue.layer),
            steps: vec![
                MissionStep {
                    id: 1,
                    description: "Lazarus pre-snapshot".into(),
                    action: None,
                    status: StepStatus::Pending,
                    result: None,
                },
                MissionStep {
                    id: 2,
                    description: format!("Fix {:?}", issue.layer),
                    action: None,
                    status: StepStatus::Pending,
                    result: None,
                },
                MissionStep {
                    id: 3,
                    description: "Validate repair".into(),
                    action: None,
                    status: StepStatus::Pending,
                    result: None,
                },
            ],
            created_at: Utc::now().to_rfc3339(),
            completed: false,
        }
    }

    async fn record_cycle(&self) {
        let palace = self.memory.lock().await;
        let _ = palace
            .store_memory(
                "SelfHealingEngine",
                "autonomic_cycle",
                "Completed diagnostic + repair pass with Arbiter gating",
            )
            .await;
    }
}
