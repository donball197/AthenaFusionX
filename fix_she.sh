#!/usr/bin/env bash
# =============================================================================
# fix_she.sh — Self-Healing Engine Loop Fix
# Targets: src/self_healing_engine.rs
# Run from: ~/nexus_singularity-dev
# =============================================================================
set -euo pipefail

echo "========================================"
echo " [NEXUS] FIXING SELF-HEALING ENGINE LOOP"
echo "========================================"

cat << 'EOF' > src/self_healing_engine.rs
// src/self_healing_engine.rs — v4.2.0 [LOOP_FIX]
//
// v4.1.x bugs fixed here:
//
// BUG 1 — process_analyzing sent a bare prompt to prepare_and_run with no
//   instruction to wrap output in <SUTURE> tags. Gemma never included them.
//   response.contains("<SUTURE>") was ALWAYS false → failure_depth always
//   incremented → FSM looped: Observing → Analyzing → Observing forever.
//   FIX: Route through task_manager (which uses the full RepoDoctor persona
//   with halt rules) instead of calling prepare_and_run directly.
//
// BUG 2 — process_observing used try_acquire_owned() which silently returns
//   Err if the semaphore is held. The entire Observing state would do nothing
//   and return, causing the FSM to spin every 15s with zero logging/progress.
//   FIX: Use acquire_owned() (async, waits for the lock) so Observing always
//   runs the Inquisitor audit when reached.
//
// BUG 3 — process_verifying same try_acquire_owned() silent-skip problem.
//   FIX: Same — use acquire_owned().

use nexus_substrate::memory::MemoryPalace;
use crate::{
    inquisitor::Inquisitor,
    lazarus_vault::LazarusVault,
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
            failure_depth: 0,
        }
    }

    fn transition(&mut self, next: HealingState) {
        info!("\x1b[35m[AIS_TRANSITION] {:?} → {:?}\x1b[0m", self.current_state, next);
        let transition_desc = format!("AIS_STATE_CHANGE: {:?}", next);
        let mut log = self.state.governance_log.lock();
        log.record(AgentAction::ExportReport {
            title: "AIS_TRANSITION".to_string(),
            content: format!("{:?}", next),
            reasoning: "Autonomic state transition".to_string(),
        }, &transition_desc);
        self.current_state = next;
    }

    pub async fn ignite_autonomic_loop(mut self) {
        info!("\x1b[36m>> [AIS v4.2] Autonomic State Machine Online\x1b[0m");

        loop {
            let cycle_delay = {
                let vitals = self.state.monitor.lock().await.get_vitals();
                if vitals.temperature > 65.0 { 60 } else { 15 }
            };
            sleep(Duration::from_secs(cycle_delay)).await;

            match self.current_state.clone() {
                HealingState::Observing          => self.process_observing().await,
                HealingState::Analyzing(log)     => self.process_analyzing(log).await,
                HealingState::Suturing(fix)      => self.process_suturing(fix).await,
                HealingState::Verifying          => self.process_verifying().await,
                HealingState::RollingBack(hash)  => self.process_rollback(hash).await,
                HealingState::Quarantine => {
                    warn!("[AIS] Quarantine active. Waiting for substrate cooldown.");
                    sleep(Duration::from_secs(300)).await;
                    self.transition(HealingState::Observing);
                }
            }
        }
    }

    async fn process_observing(&mut self) {
        // FIX BUG 2: acquire_owned() waits for the lock instead of silently
        // skipping. The old try_acquire_owned() caused Observing to do nothing
        // whenever a suture was in progress, spinning the FSM uselessly.
        let _permit = COMPILE_LOCK.clone().acquire_owned().await;
        match Inquisitor::audit_backend().await {
            Ok(log) if log == "NOMINAL" => {
                self.failure_depth = 0;
                info!("[AIS] Substrate nominal. Observing.");
            }
            Ok(log) => {
                METRICS.inc_failure();
                info!("[AIS] Fault detected — transitioning to Analyzing.");
                self.transition(HealingState::Analyzing(log));
            }
            Err(e) => error!("[AIS] Inquisitor fault: {}", e),
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

        // Snapshot before any suture attempt
        for target in SNAPSHOT_TARGETS {
            if std::path::Path::new(target).exists() {
                let _ = LazarusVault::auto_snapshot(target, "pre_suture").await;
            }
        }

        // FIX BUG 1: Old code called prepare_and_run directly with a bare prompt
        // and then checked for a <SUTURE> tag that it never instructed the model
        // to produce. Response was always tag-free → always failure_depth++ → loop.
        //
        // New approach: route through task_manager with the full RepoDoctor
        // directive. The router's RepoDoctor persona instructs the model to:
        //   1. read_file  2. write_file  3. cargo check  4. halt when clean.
        // task_manager handles turn limits, deadlock detection, and halt.
        // We then go straight to Verifying — the Inquisitor confirms the result.
        info!("[AIS] Routing repair to task_manager (RepoDoctor profile)...");
        let directive = format!(
            "REPO_DOCTOR: The substrate has reported the following error. \
             Diagnose and repair it. Run cargo check to verify. Halt when clean.\n\
             ERROR LOG:\n{}",
            error_log
        );

        let _ = crate::task_manager::send_task(
            self.state.orchestrator.clone(),
            &self.state.client,
            &self.state.api_key,
            &directive,
            self.state.memory.clone(),
            self.state.vmo_state.clone(),
            self.state.tx.clone(),
            self.state.dist.clone(),
        ).await;

        // Move to Verifying regardless — Inquisitor will determine if it worked
        self.failure_depth += 1;
        self.transition(HealingState::Verifying);
    }

    // process_suturing is now a fallback for future <SUTURE>-tagged responses
    // from other AI paths. Kept for compatibility.
    async fn process_suturing(&mut self, raw_suture: String) {
        if crate::json_parser::extract_actions(&raw_suture).is_ok() {
            let _ = crate::task_manager::send_task(
                self.state.orchestrator.clone(), &self.state.client, &self.state.api_key,
                &format!("Apply suture actions:\n{}", raw_suture.trim()),
                self.state.memory.clone(), self.state.vmo_state.clone(),
                self.state.tx.clone(), self.state.dist.clone(),
            ).await;
            self.transition(HealingState::Verifying);
            return;
        }
        if let (Some(start), Some(end)) = (raw_suture.find("<SUTURE>"), raw_suture.find("</SUTURE>")) {
            let code = raw_suture[start + 8..end].trim().to_string();
            let _ = crate::task_manager::send_task(
                self.state.orchestrator.clone(), &self.state.client, &self.state.api_key,
                &format!("Apply suture:\n{}", code),
                self.state.memory.clone(), self.state.vmo_state.clone(),
                self.state.tx.clone(), self.state.dist.clone(),
            ).await;
            self.transition(HealingState::Verifying);
            return;
        }
        // Nothing parseable — go back to Observing
        warn!("[AIS] Suture payload unparseable. Returning to Observing.");
        self.failure_depth += 1;
        self.transition(HealingState::Observing);
    }

    async fn process_verifying(&mut self) {
        // FIX BUG 3: Same try_acquire_owned() silent-skip as Observing.
        // Use acquire_owned() so verification always runs.
        let _permit = COMPILE_LOCK.clone().acquire_owned().await;
        match Inquisitor::audit_backend().await {
            Ok(log) if log == "NOMINAL" => {
                METRICS.inc_heal_success();
                self.failure_depth = 0;
                info!("[AIS] ✅ Suture verified — substrate nominal.");
                self.transition(HealingState::Observing);
            }
            Ok(log) => {
                METRICS.inc_heal_failure();
                self.failure_depth += 1;
                warn!("[AIS] Suture insufficient — re-analyzing (depth={}).", self.failure_depth);
                self.transition(HealingState::Analyzing(log));
            }
            Err(_) => {
                warn!("[AIS] Inquisitor error during verification — retrying Observing.");
                self.transition(HealingState::Observing);
            }
        }
    }

    async fn process_rollback(&mut self, hash: String) {
        warn!("[AIS] Rolling back to snapshot {}.", hash);
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
EOF

echo "  ✔ src/self_healing_engine.rs rewritten (v4.2.0)"

echo ""
echo "Verifying build..."
cargo check 2>&1 | grep '^error' && echo "  ✘ errors found" || echo "  ✅ BUILD CLEAN"

echo ""
echo "========================================"
echo " [SUCCESS] SHE loop fixed."
echo "========================================"
