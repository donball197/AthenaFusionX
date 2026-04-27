// src/self_healing_engine.rs — v4.1 [RESOURCE_LOCKED + GOVERNED]
use crate::{
    inquisitor::Inquisitor,
    lazarus_vault::LazarusVault,
    memory::MemoryPalace,
    neural_agent::ContextPacket,
    server::AppState,
    metrics::METRICS,
    agent_protocol::AgentAction,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Semaphore, Mutex};
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

static COMPILE_LOCK: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

const SNAPSHOT_TARGETS: &[&str] = &[
    "src/main.rs",
    "src/lib.rs",
    "src/executor.rs",
    "src/task_manager.rs",
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

pub struct SelfHealingEngine {
    state: Arc<AppState>,
    current_state: HealingState,
    failure_depth: u8,
}

impl SelfHealingEngine {
    pub fn new(state: Arc<AppState>, _memory: Arc<Mutex<MemoryPalace>>) -> Self {
        Self { 
            state, 
            current_state: HealingState::Observing,
            failure_depth: 0 
        }
    }

    fn transition(&mut self, next: HealingState) {
        info!("\x1b[35m[AIS_TRANSITION] {:?} → {:?}\x1b[0m", self.current_state, next);
        
        // ⚡ PHASE 2.5: Cryptographic Governance Logging
        // We log the transition as a SystemAction to the Merkle Tree
        let transition_desc = format!("AIS_STATE_CHANGE: {:?}", next);
        let mut log = self.state.governance_log.lock();
        log.record(AgentAction::Halt, &transition_desc); // Using Halt as a placeholder for state sync
        
        self.current_state = next;
    }

    pub async fn ignite_autonomic_loop(mut self) {
        info!("\x1b[36m>> [AIS v4.1] Autonomic State Machine Online (Locked)\x1b[0m");

        loop {
            let vitals = self.state.monitor.lock().await.get_vitals();
            let cycle_delay = if vitals.temperature > 65.0 { 60 } else { 15 };
            sleep(Duration::from_secs(cycle_delay)).await;

            match self.current_state.clone() {
                HealingState::Observing => self.process_observing().await,
                HealingState::Analyzing(log) => self.process_analyzing(log).await,
                HealingState::Suturing(fix) => self.process_suturing(fix).await,
                HealingState::Verifying => self.process_verifying().await,
                HealingState::RollingBack(hash) => self.process_rollback(hash).await,
                HealingState::Quarantine => {
                    warn!("[AIS] Quarantine active. Waiting for substrate cooldown.");
                    sleep(Duration::from_secs(300)).await;
                    self.transition(HealingState::Observing);
                }
            }
        }
    }

    async fn process_observing(&mut self) {
        // ⚡ FIX: Actually utilizing COMPILE_LOCK to prevent OOM during audit
        if let Ok(_permit) = COMPILE_LOCK.clone().try_acquire_owned() {
            match Inquisitor::audit_backend().await {
                Ok(log) if log == "NOMINAL" => { self.failure_depth = 0; }
                Ok(log) => {
                    METRICS.inc_failure();
                    self.transition(HealingState::Analyzing(log));
                }
                Err(e) => error!("[AIS] Inquisitor fault: {}", e),
            }
        }
    }

    async fn process_analyzing(&mut self, error_log: String) {
        if self.failure_depth >= 3 {
            let snapshots = LazarusVault::list_snapshots();
            if let Some(stable) = snapshots.last() {
                self.transition(HealingState::RollingBack(stable.hash.clone()));
            } else {
                self.transition(HealingState::Quarantine);
            }
            return;
        }

        for target in SNAPSHOT_TARGETS {
            if std::path::Path::new(target).exists() {
                let _ = LazarusVault::auto_snapshot(target, "pre_suture").await;
            }
        }

        let prompt = format!("IDENTITY: NEXUS REPO DOCTOR. Fix this: {}", error_log);
        match self.state.agent.prepare_and_run(
            &self.state.client, &self.state.api_key, &prompt, ContextPacket::default(), 0.1, Some("gemma-4-31b-it")
        ).await {
            Ok((response, _)) => {
                if response.contains("<SUTURE>") { self.transition(HealingState::Suturing(response)); }
                else { self.failure_depth += 1; self.transition(HealingState::Observing); }
            }
            Err(_) => { self.failure_depth += 1; self.transition(HealingState::Observing); }
        }
    }

    async fn process_suturing(&mut self, raw_suture: String) {
        if let Some(start) = raw_suture.find("<SUTURE>") {
            if let Some(end) = raw_suture.find("</SUTURE>") {
                let code = &raw_suture[start + 8..end].trim();
                let _ = crate::task_manager::send_task(
                    self.state.orchestrator.clone(), &self.state.client, &self.state.api_key,
                    &format!("Apply suture:\n{}", code),
                    self.state.memory.clone(), self.state.vmo_state.clone(),
                    self.state.tx.clone(), self.state.dist.clone(),
                ).await;
                self.transition(HealingState::Verifying);
                return;
            }
        }
        self.transition(HealingState::Observing);
    }

    async fn process_verifying(&mut self) {
        if let Ok(_permit) = COMPILE_LOCK.clone().try_acquire_owned() {
            match Inquisitor::audit_backend().await {
                Ok(log) if log == "NOMINAL" => {
                    METRICS.inc_heal_success();
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
        let _ = crate::task_manager::send_task(
            self.state.orchestrator.clone(), &self.state.client, &self.state.api_key,
            &format!("restore snapshot {}", hash),
            self.state.memory.clone(), self.state.vmo_state.clone(),
            self.state.tx.clone(), self.state.dist.clone(),
        ).await;
        self.failure_depth = 0;
        self.transition(HealingState::Observing);
    }
}
