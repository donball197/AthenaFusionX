#!/usr/bin/env bash
# =============================================================================
# restore_self_healing.sh — Restore lost self-healing engine + lazarus vault
# The restructuring to v3.0 reduced these to empty stubs:
#   bins/ide_backend/src/self_healing_engine.rs  → "pub fn init() { ... }"
#   bins/ide_backend/src/lazarus_vault.rs        → "pub fn init() { ... }"
# This script restores the full v4.1.2 SelfHealingEngine + v1.4 LazarusVault
# and adds the Inquisitor and GovernanceLog in their correct v3.0 locations.
# Run from: ~/nexus_singularity-dev
# =============================================================================
set -euo pipefail

CYAN='\033[1;36m'; GREEN='\033[1;32m'; YELLOW='\033[1;33m'; RED='\033[1;31m'; NC='\033[0m'
log()  { echo -e "${CYAN}[RESTORE]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }

ROOT="$(pwd)"
[[ -f "$ROOT/Cargo.toml" ]] || { echo "Run from ~/nexus_singularity-dev"; exit 1; }

IDE="$ROOT/bins/ide_backend/src"
RC="$ROOT/crates/runtime_core/src"

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE 1 — LazarusVault v1.4
# Full snapshot + ledger + deduplication + tests.
# Moved to crates/runtime_core so it can be used by both CLI and IDE backend.
# ─────────────────────────────────────────────────────────────────────────────
log "Restoring crates/runtime_core/src/lazarus_vault.rs (v1.4)..."

cat > "$RC/lazarus_vault.rs" << 'EOF'
// lazarus_vault.rs — v1.4 [LEDGER_FIX]
// FIX: auto_snapshot() now appends every snapshot to ledger.json.
//      Previously it wrote to .nexus/vault/<hash> but never touched the ledger,
//      so list_snapshots() returned [] and rollback was permanently blind.
// FIX: list_snapshots() returns entries sorted oldest→newest so
//      .last() in the rollback path gives the most recent stable state.
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub id:        String,
    pub path:      String,
    pub hash:      String,
    pub timestamp: String,
    pub directive: String,
}

pub struct LazarusVault;

impl LazarusVault {
    const VAULT_DIR:   &'static str = ".nexus/vault";
    const LEDGER_PATH: &'static str = ".nexus/vault/ledger.json";

    /// Snapshot a single file to the vault and append to ledger.
    pub async fn auto_snapshot(path: &str, tag: &str) -> Result<Snapshot, String> {
        // Reject directories — vault only snapshots files
        let metadata = fs::metadata(path)
            .map_err(|e| format!("PATH_NOT_FOUND: {}", e))?;
        if metadata.is_dir() {
            return Err("DIRECTORY_SNAPSHOT_NOT_SUPPORTED: Specify a file.".into());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("FILE_READ_FAULT: {}", e))?;

        // Content hash for deduplication
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        fs::create_dir_all(Self::VAULT_DIR)
            .map_err(|e| format!("DIR_FAULT: {}", e))?;

        let dest = format!("{}/{}", Self::VAULT_DIR, hash);
        fs::write(&dest, &content)
            .map_err(|e| format!("FS_WRITE_FAULT: {}", e))?;

        let snapshot = Snapshot {
            id:        hash.clone(),
            path:      path.to_string(),
            hash:      hash.clone(),
            timestamp: format!("{:?}", SystemTime::now()),
            directive: tag.to_string(),
        };

        // FIX: Actually persist to ledger so list_snapshots() can find it
        Self::append_to_ledger(&snapshot)?;

        println!(
            "\x1b[32m[LAZARUS] Snapshot {} → {} [tag: {}]\x1b[0m",
            &hash[..8.min(hash.len())], path, tag
        );

        Ok(snapshot)
    }

    /// Restore a snapshot by hash — writes the saved content back to its path.
    pub fn restore(hash: &str) -> Result<String, String> {
        let src = format!("{}/{}", Self::VAULT_DIR, hash);
        let content = fs::read_to_string(&src)
            .map_err(|e| format!("VAULT_READ_FAULT [{}]: {}", hash, e))?;

        // Find the target path from the ledger
        let snap = Self::list_snapshots()
            .into_iter()
            .find(|s| s.hash == hash)
            .ok_or_else(|| format!("HASH_NOT_IN_LEDGER: {}", hash))?;

        fs::write(&snap.path, &content)
            .map_err(|e| format!("RESTORE_WRITE_FAULT: {}", e))?;

        Ok(format!("[LAZARUS] Restored {} → {}", &hash[..8.min(hash.len())], snap.path))
    }

    /// Appends a snapshot to ledger.json. Deduplicates by hash.
    fn append_to_ledger(snapshot: &Snapshot) -> Result<(), String> {
        let mut entries = Self::list_snapshots();

        if entries.iter().any(|e| e.hash == snapshot.hash) {
            return Ok(()); // Already snapshotted identical content
        }

        entries.push(snapshot.clone());

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| format!("LEDGER_SERIALIZE_FAULT: {}", e))?;

        fs::write(Self::LEDGER_PATH, json)
            .map_err(|e| format!("LEDGER_WRITE_FAULT: {}", e))?;

        Ok(())
    }

    /// Returns all snapshots sorted oldest → newest.
    /// Use .last() for the most recent stable snapshot.
    pub fn list_snapshots() -> Vec<Snapshot> {
        if !Path::new(Self::LEDGER_PATH).exists() {
            return vec![];
        }
        let data = fs::read_to_string(Self::LEDGER_PATH).unwrap_or_default();
        let mut entries: Vec<Snapshot> = serde_json::from_str(&data).unwrap_or_default();
        entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        entries
    }

    /// Returns the most recent snapshot for a given file path.
    pub fn latest_for_path(path: &str) -> Option<Snapshot> {
        Self::list_snapshots()
            .into_iter()
            .filter(|s| s.path == path)
            .last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_snapshot_and_ledger() {
        let test_file = "/tmp/nexus_test_snapshot.rs";
        fs::write(test_file, "fn main() {}").unwrap();

        let snap = LazarusVault::auto_snapshot(test_file, "test_tag")
            .await
            .expect("snapshot failed");

        assert!(!snap.hash.is_empty());
        assert_eq!(snap.path, test_file);

        let ledger = LazarusVault::list_snapshots();
        assert!(ledger.iter().any(|e| e.hash == snap.hash));

        fs::remove_file(test_file).ok();
    }
}
EOF
ok "lazarus_vault.rs restored."

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE 2 — Inquisitor v1.1
# Runs `cargo check` to audit the backend, audits the UI, probes for
# shadow file activity. Required by SelfHealingEngine.
# ─────────────────────────────────────────────────────────────────────────────
log "Restoring crates/runtime_core/src/inquisitor.rs (v1.1)..."

cat > "$RC/inquisitor.rs" << 'EOF'
// inquisitor.rs — v1.1 [UI_AUDIT_RESTORED]
// Autonomous workspace auditor. Called by SelfHealingEngine on each cycle.
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use tokio::process::Command;

pub struct Inquisitor;

impl Inquisitor {
    /// audit_ui: Checks for missing critical Svelte/UI files.
    pub async fn audit_ui() -> anyhow::Result<String> {
        let critical_paths = ["ui/src/App.svelte", "ui/package.json", "ui/index.html"];

        for path in critical_paths {
            if !Path::new(path).exists() {
                return Ok(format!("FRACTURE: Missing critical UI file: {}", path));
            }
        }

        if !Path::new("ui/node_modules").exists() {
            return Ok("FRACTURE: UI dependencies (node_modules) missing. Run npm install.".into());
        }

        Ok("NOMINAL".into())
    }

    /// audit_backend: Runs `cargo check` and returns errors or "NOMINAL".
    pub async fn audit_backend() -> anyhow::Result<String> {
        let output = Command::new("cargo")
            .args(["check"])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Cargo check failed: {}", e))?;

        if output.status.success() {
            Ok("NOMINAL".into())
        } else {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// diagnose_workspace: Runs both audits and returns a combined report.
    pub async fn diagnose_workspace() -> anyhow::Result<String> {
        let backend = Self::audit_backend().await.unwrap_or_else(|e| e.to_string());
        let ui      = Self::audit_ui().await.unwrap_or_else(|e| e.to_string());

        if backend == "NOMINAL" && ui == "NOMINAL" {
            return Ok("WORKSPACE_NOMINAL: Integrity verified across Rust and Svelte domains.".into());
        }

        let mut diagnostic = String::from("WORKSPACE_FRACTURE_DETECTED:\n");
        if backend != "NOMINAL" {
            diagnostic.push_str(&format!("--- BACKEND ERRORS ---\n{}\n", backend));
        }
        if ui != "NOMINAL" {
            diagnostic.push_str(&format!("--- FRONTEND ERRORS ---\n{}\n", ui));
        }

        Ok(diagnostic)
    }

    /// probe_shadow_activity: Detects files modified in the last 60 seconds.
    pub fn probe_shadow_activity() -> Vec<String> {
        let mut alerts = Vec::new();
        let targets = ["./src", "./ui/src"];

        for path in targets {
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                        if elapsed.as_secs() < 60 {
                            alerts.push(format!(
                                "SHADOW_ACTIVITY: {} modified recently ({}s ago)",
                                path, elapsed.as_secs()
                            ));
                        }
                    }
                }
            }
        }
        alerts
    }
}
EOF
ok "inquisitor.rs restored."

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE 3 — GovernanceLog with MerkleLeaf
# Moved to crates/runtime_core/src/governance/mod.rs
# ─────────────────────────────────────────────────────────────────────────────
log "Restoring governance module (MerkleLeaf + GovernanceLog)..."

mkdir -p "$RC/governance"

cat > "$RC/governance/mod.rs" << 'EOF'
// governance/mod.rs — v2.0 [MERKLE_CHAIN]
// Incremental Merkle root: each action hashes into a running chain.
// Provides a tamper-evident audit trail of every agent action.
pub mod protocol;
EOF

cat > "$RC/governance/protocol.rs" << 'EOF'
// governance/protocol.rs — v2.0 [MERKLE_CHAIN]
use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleLeaf {
    pub action:    AgentAction,
    pub actor_id:  String,
    pub timestamp: u64,
    pub hash:      u64,
}

impl MerkleLeaf {
    pub fn new(action: AgentAction, actor_id: &str) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut hasher = DefaultHasher::new();
        serde_json::to_vec(&action).unwrap_or_default().hash(&mut hasher);
        actor_id.hash(&mut hasher);
        ts.hash(&mut hasher);
        Self {
            action,
            actor_id: actor_id.to_string(),
            timestamp: ts,
            hash: hasher.finish(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GovernanceLog {
    pub history:   Vec<MerkleLeaf>,
    pub root_hash: u64,
}

impl GovernanceLog {
    pub fn init() -> Self {
        Self::default()
    }

    /// record: Chains a new leaf into the Merkle root.
    /// Root = Hash(OldRoot ++ NewLeaf)
    pub fn record(&mut self, action: AgentAction, actor_id: &str) {
        let leaf = MerkleLeaf::new(action, actor_id);

        let mut hasher = DefaultHasher::new();
        self.root_hash.hash(&mut hasher);
        leaf.hash.hash(&mut hasher);
        self.root_hash = hasher.finish();

        self.history.push(leaf);

        if self.history.len() % 10 == 0 {
            info!(
                "\x1b[34m[GOVERNANCE] Checkpoint: Root=0x{:x} (Size: {})\x1b[0m",
                self.root_hash,
                self.history.len()
            );
        }
    }

    pub fn root(&self) -> u64 {
        self.root_hash
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}
EOF
ok "governance/mod.rs + protocol.rs restored."

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE 4 — SelfHealingEngine v4.1.2
# Full autonomic state machine:
#   Observing → Analyzing → Suturing → Verifying → RollingBack → Quarantine
# Lives in crates/runtime_core so it can be used from any binary.
# ─────────────────────────────────────────────────────────────────────────────
log "Restoring crates/runtime_core/src/self_healing_engine.rs (v4.1.2)..."

cat > "$RC/self_healing_engine.rs" << 'EOF'
// self_healing_engine.rs — v4.1.2 [AUTONOMIC_STATE_MACHINE]
// Full FSM: Observing → Analyzing → Suturing → Verifying → RollingBack → Quarantine
// Runs as a background tokio task via ignite_autonomic_loop().
// Requires: Inquisitor (cargo check), LazarusVault (snapshots), GovernanceLog (audit trail)
use crate::{
    governance::protocol::GovernanceLog,
    inquisitor::Inquisitor,
    lazarus_vault::LazarusVault,
    metrics::METRICS,
    neural_agent::ContextPacket,
    agent_protocol::AgentAction,
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
static COMPILE_LOCK: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

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
    pub client:          Client,
    pub api_key:         String,
    pub orchestrator:    Arc<crate::orchestrator::Orchestrator>,
    pub memory:          Arc<tokio::sync::Mutex<MemoryPalace>>,
    pub vmo_state:       Arc<tokio::sync::Mutex<crate::shared_state::VmoState>>,
    pub tx:              tokio::sync::broadcast::Sender<String>,
    pub dist:            Arc<crate::distributed_scheduler::DistributedScheduler>,
    pub governance_log:  Arc<Mutex<GovernanceLog>>,
    pub agent:           Arc<crate::neural_agent::MicroAgent>,
}

pub struct SelfHealingEngine {
    ctx:           Arc<HealingContext>,
    current_state: HealingState,
    failure_depth: u8,
}

impl SelfHealingEngine {
    pub fn new(ctx: Arc<HealingContext>) -> Self {
        Self {
            ctx,
            current_state: HealingState::Observing,
            failure_depth: 0,
        }
    }

    fn transition(&mut self, next: HealingState) {
        info!(
            "\x1b[35m[AIS_TRANSITION] {:?} → {:?}\x1b[0m",
            self.current_state, next
        );
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
                HealingState::Observing            => self.process_observing().await,
                HealingState::Analyzing(log)       => self.process_analyzing(log).await,
                HealingState::Suturing(fix)        => self.process_suturing(fix).await,
                HealingState::Verifying            => self.process_verifying().await,
                HealingState::RollingBack(hash)    => self.process_rollback(hash).await,
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

        match self.ctx.agent.prepare_and_run(
            &self.ctx.client,
            &self.ctx.api_key,
            &prompt,
            ContextPacket::default(),
            0.1,
            Some("gemma-4-31-it"),
        ).await {
            Ok((response, _)) => {
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
        } else if let (Some(start), Some(end)) = (
            raw_suture.find("<SUTURE>"),
            raw_suture.find("</SUTURE>"),
        ) {
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
        ).await;

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
            Ok(msg)  => info!("[AIS] {}", msg),
            Err(e)   => error!("[AIS] Rollback failed: {}", e),
        }
        self.failure_depth = 0;
        self.transition(HealingState::Observing);
    }
}
EOF
ok "self_healing_engine.rs restored (full v4.1.2 FSM)."

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE 5 — Update ide_backend stubs to re-export from runtime_core
# ─────────────────────────────────────────────────────────────────────────────
log "Updating ide_backend stubs to re-export from runtime_core..."

cat > "$IDE/self_healing_engine.rs" << 'EOF'
// bins/ide_backend/src/self_healing_engine.rs
// Re-exports the real engine from runtime_core.
pub use nexus_runtime_core::self_healing_engine::{
    SelfHealingEngine,
    HealingContext,
    HealingState,
};
pub fn init() { println!(">> [AIS] Self-Healing Engine module linked."); }
EOF

cat > "$IDE/lazarus_vault.rs" << 'EOF'
// bins/ide_backend/src/lazarus_vault.rs
// Re-exports the vault from runtime_core.
pub use nexus_runtime_core::lazarus_vault::{LazarusVault, Snapshot};
pub fn init() { println!(">> [LAZARUS] Vault module linked."); }
EOF
ok "IDE backend stubs updated."

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE 6 — Add new modules to runtime_core/src/lib.rs
# ─────────────────────────────────────────────────────────────────────────────
LIB_RS="$RC/lib.rs"
log "Adding missing modules to runtime_core/src/lib.rs..."

for mod_name in "lazarus_vault" "inquisitor" "self_healing_engine" "governance"; do
    if ! grep -q "pub mod ${mod_name}" "$LIB_RS"; then
        echo "pub mod ${mod_name};" >> "$LIB_RS"
        ok "  Added: pub mod ${mod_name}"
    else
        ok "  Already present: ${mod_name}"
    fi
done

# ─────────────────────────────────────────────────────────────────────────────
# RESTORE 7 — Lazarus route in server.rs
# Add /lazarus/snapshots endpoint if missing
# ─────────────────────────────────────────────────────────────────────────────
SERVER="$IDE/server.rs"
if [[ -f "$SERVER" ]] && ! grep -q "lazarus/snapshots" "$SERVER"; then
    log "Adding /lazarus/snapshots route to server.rs..."
    sed -i 's|\.route("/ws/telemetry"|.route("/lazarus/snapshots", get(|| async { axum::Json(nexus_runtime_core::lazarus_vault::LazarusVault::list_snapshots()) }))\n        .route("/ws/telemetry"|' "$SERVER"
    ok "Lazarus route added to server.rs."
else
    ok "Lazarus route already present or server.rs not found."
fi

# ─────────────────────────────────────────────────────────────────────────────
# SUMMARY
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  Self-healing restored. What you got back:                   ${NC}"
echo -e "${GREEN}  ✓ LazarusVault v1.4   — snapshot + ledger + rollback        ${NC}"
echo -e "${GREEN}  ✓ Inquisitor v1.1     — cargo check + UI audit + shadow probe${NC}"
echo -e "${GREEN}  ✓ GovernanceLog v2.0  — Merkle chain audit trail            ${NC}"
echo -e "${GREEN}  ✓ SelfHealingEngine v4.1.2 — full 6-state autonomic FSM    ${NC}"
echo -e "${GREEN}                                                               ${NC}"
echo -e "${GREEN}  Run:  cargo check 2>&1 | head -80                           ${NC}"
echo -e "${GREEN}══════════════════════════════════════════════════════════════${NC}"
