// self_healing_engine.rs — v4.1.2 [AUTONOMIC_STATE_MACHINE]
// Full FSM: Observing → Analyzing → Suturing → Verifying → RollingBack → Quarantine
// Runs as a background tokio task via ignite_autonomic_loop().
// Requires: Inquisitor (cargo check), LazarusVault (snapshots), GovernanceLog (audit trail)
use crate::{
    agent_protocol::AgentAction, governance::protocol::GovernanceLog, inquisitor::Inquisitor,
    lazarus_vault::LazarusVault, metrics::METRICS, 
};
use nexus_substrate::memory::MemoryPalace;
use parking_lot::Mutex;
use reqwest::Client;
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

/// Only one cargo check runs at a time — prevents runaway compile processes on
/// the 4GB Duet where concurrent compiles thrash RAM and eMMC swap.
static COMPILE_LOCK: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

/// Files snapshotted before every suture attempt so rollback is always possible.
const SNAPSHOT_TARGETS: &[&str] = &[
    "crates/runtime_core/src/executor.rs",
    "crates/runtime_core/src/task_manager.rs",
    "bins/ide_backend/src/server.rs",
    "bins/ide_backend/src/main.rs",
];

#[derive(Debug, Clone, PartialEq)]
pub enum HealingState {
    Observing,
    Analyzing(String),
    Suturing(String),
    Verifying,
    RollingBack(String),
    Quarantine,
}

/// Shared context the engine needs to call the API and manage tasks.
/// Mirrors the fields from AppState that the engine actually uses.
pub struct HealingContext {
    pub client: Client,
    pub api_key: String,
    pub orchestrator: Arc<crate::orchestrator::Orchestrator>,
    pub memory: Arc<tokio::sync::Mutex<MemoryPalace>>,
    pub vmo_state: Arc<tokio::sync::Mutex<crate::shared_state::VmoState>>,
    pub tx: tokio::sync::broadcast::Sender<String>,
    pub dist: Arc<crate::distributed_scheduler::DistributedScheduler>,
    pub governance_log: Arc<Mutex<GovernanceLog>>,
    pub agent: Arc<crate::neural_agent::MicroAgent>,
}

pub struct SelfHealingEngine {
    ctx: Arc<HealingContext>,
    current_state: HealingState,
    failure_depth: u8,
}

impl SelfHealingEngine {
    pub fn new(ctx: Arc<HealingContext>) -> Self {
        Self { ctx, current_state: HealingState::Observing, failure_depth: 0 }
    }

    fn transition(&mut self, next: HealingState) {
        info!("\x1b[35m[AIS_TRANSITION] {:?} → {:?}\x1b[0m", self.current_state, next);
        let mut log = self.ctx.governance_log.lock();
        log.record(
            AgentAction::ExportReport {
                title: "AIS_TRANSITION".to_string(),
                content: format!("{:?}", next),
                reasoning: "Autonomic state transition".to_string(),
            },
            "SelfHealingEngine",
        );
        self.current_state = next;
    }

    /// Entry point — run this in a dedicated tokio::spawn().
    pub async fn ignite_autonomic_loop(mut self) {
        info!("\x1b[36m>> [AIS v4.1] Autonomic State Machine Online\x1b[0m");

        loop {
            // Thermal throttling: slow down the cycle when the CPU is hot
            // (critical on the Lenovo Duet which throttles at ~70°C)
            let cycle_delay = 15u64;
            sleep(Duration::from_secs(cycle_delay)).await;

            match self.current_state.clone() {
                HealingState::Observing => self.process_observing().await,
                HealingState::Analyzing(log) => self.process_analyzing(log).await,
                HealingState::Suturing(fix) => self.process_suturing(fix).await,
                HealingState::Verifying => self.process_verifying().await,
                HealingState::RollingBack(hash) => self.process_rollback(hash).await,
                HealingState::Quarantine => {
                    warn!("[AIS] Quarantine active. Waiting 5 minutes for cooldown.");
                    sleep(Duration::from_secs(300)).await;
                    self.failure_depth = 0;
                    self.transition(HealingState::Observing);
                }
            }
        }
    }

    async fn process_observing(&mut self) {
        // Only audit if we can acquire the compile lock (don't stack checks)
        if let Ok(_permit) = COMPILE_LOCK.clone().try_acquire_owned() {
            match Inquisitor::audit_backend().await {
                Ok(log) if log == "NOMINAL" => {
                    self.failure_depth = 0;
                }
                Ok(log) => {
                    METRICS.inc_failure();
                    self.transition(HealingState::Analyzing(log));
                }
                Err(e) => error!("[AIS] Inquisitor fault: {}", e),
            }
        }
    }

    async fn process_analyzing(&mut self, error_log: String) {
        // After 3 failed suture attempts, escalate to rollback
        if self.failure_depth >= 3 {
            let snapshots = LazarusVault::list_snapshots();
            if let Some(stable) = snapshots.last() {
                self.transition(HealingState::RollingBack(stable.hash.clone()));
            } else {
                self.transition(HealingState::Quarantine);
            }
            return;
        }

        // Snapshot all critical files before attempting a suture
        for target in SNAPSHOT_TARGETS {
            if std::path::Path::new(target).exists() {
                let _ = LazarusVault::auto_snapshot(target, "pre_suture").await;
            }
        }

        let prompt = format!(
            "IDENTITY: NEXUS REPO DOCTOR. You MUST output ONLY a valid JSON array. Fix this compilation error:\n{}",
            error_log
        );

        let orch = crate::orchestrator::Orchestrator::new();
        match orch.run(
            prompt.clone(),
            crate::neural_agent::ContextPacket {
                model_override: Some("models/gemma-4-26b-a4b-it".to_string()),
                ..Default::default()
            },
            &self.ctx.client,
            &self.ctx.api_key,
            None,
        ).await.map_err(|e| anyhow::anyhow!(e)) {
            Ok(response) => {
                if response.contains("<SUTURE>") || response.trim_start().starts_with('[') {
                    self.transition(HealingState::Suturing(response));
                } else {
                    self.failure_depth += 1;
                    self.transition(HealingState::Observing);
                }
            }
            Err(e) => {
                error!("[AIS] Agent call failed: {}", e);
                self.failure_depth += 1;
                self.transition(HealingState::Observing);
            }
        }
    }

    async fn process_suturing(&mut self, raw_suture: String) {
        // Try direct JSON array first
        let suture_payload = if crate::json_parser::extract_actions(&raw_suture).is_ok() {
            raw_suture.trim().to_string()
        } else if let (Some(start), Some(end)) =
            (raw_suture.find("<SUTURE>"), raw_suture.find("</SUTURE>"))
        {
            raw_suture[start + 8..end].trim().to_string()
        } else {
            self.failure_depth += 1;
            self.transition(HealingState::Observing);
            return;
        };

        let _ = crate::task_manager::send_task(
            self.ctx.orchestrator.clone(),
            &self.ctx.client,
            &self.ctx.api_key,
            &format!("Apply suture actions:\n{}", suture_payload),
            self.ctx.memory.clone(),
            self.ctx.vmo_state.clone(),
            self.ctx.tx.clone(),
            self.ctx.dist.clone(),
        )
        .await;

        self.transition(HealingState::Verifying);
    }

    async fn process_verifying(&mut self) {
        if let Ok(_permit) = COMPILE_LOCK.clone().try_acquire_owned() {
            match Inquisitor::audit_backend().await {
                Ok(log) if log == "NOMINAL" => {
                    METRICS.inc_heal_success();
                    info!("\x1b[32m[AIS] Suture successful — workspace nominal.\x1b[0m");
                    self.failure_depth = 0;
                    self.transition(HealingState::Observing);
                }
                Ok(log) => {
                    METRICS.inc_heal_failure();
                    self.failure_depth += 1;
                    self.transition(HealingState::Analyzing(log));
                }
                Err(_) => self.transition(HealingState::Observing),
            }
        }
    }

    async fn process_rollback(&mut self, hash: String) {
        warn!("[AIS] Rolling back to snapshot: {}...", &hash[..8.min(hash.len())]);
        match LazarusVault::restore(&hash) {
            Ok(msg) => info!("[AIS] {}", msg),
            Err(e) => error!("[AIS] Rollback failed: {}", e),
        }
        self.failure_depth = 0;
        self.transition(HealingState::Observing);
    }
}
