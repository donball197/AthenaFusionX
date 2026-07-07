#!/usr/bin/env python3
"""
wire_modes_and_governance.py

Four coordinated changes, verified-then-applied atomically (all anchors
checked before ANY file is written, to avoid a half-patched state):

1. governance/protocol.rs — adds GOVERNANCE: LazyLock<Mutex<GovernanceLog>>.
   Previously task_manager.rs did `GovernanceLog::init()` fresh on every
   call, so even its one record() was thrown away with the instance.

2. executor.rs — RoboticExecutor::execute_single_action and
   ToolExecutor::execute now record every successful action to
   GOVERNANCE. This is the real chokepoint: it catches actions from
   execute_handle, CorticalLoop's direct per-action calls, AND the
   ReAct path, without needing to touch cortical_loop.rs or react_agent.rs.

3. task_manager.rs — explicit ExecutionMode enum (GoT / ReAct / Cortical)
   replaces the old silent try-v4-then-fallback-to-cortical chain.

4. bins/cli/src/main.rs — "/v4 <text>" and "/react <text>" prefixes route
   to those modes explicitly; plain directive still defaults to Cortical
   (matches current live fallback behavior).
"""
import sys

FILES = {}

# ─────────────────────────────────────────────────────────────────────
# 1. governance/protocol.rs
# ─────────────────────────────────────────────────────────────────────
FILES["crates/runtime_core/src/governance/protocol.rs"] = {
    # NOTE: anchor deliberately starts AFTER the header comment line, which
    # contains an em-dash. A prior edit through nano on the user's machine
    # re-encoded that character, breaking byte-exact matching even though
    # both files looked identical when printed. Anchoring on pure ASCII
    # sidesteps the whole class of problem; the header line is left
    # untouched by this patch either way.
    "anchor": '''use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::info;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleLeaf {
    pub action: AgentAction,
    pub actor_id: String,
    pub timestamp: u64,
    pub hash: u64,
}
impl MerkleLeaf {
    pub fn new(action: AgentAction, actor_id: &str) -> Self {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut hasher = DefaultHasher::new();
        serde_json::to_vec(&action).unwrap_or_default().hash(&mut hasher);
        actor_id.hash(&mut hasher);
        ts.hash(&mut hasher);
        Self { action, actor_id: actor_id.to_string(), timestamp: ts, hash: hasher.finish() }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GovernanceLog {
    pub history: Vec<MerkleLeaf>,
    pub root_hash: u64,
}
impl GovernanceLog {
    pub fn init() -> Self {
        Self::default()
    }
    pub fn init_shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::init()))
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
        if self.history.len().is_multiple_of(10) {
            info!(
                "[GOVERNANCE] Checkpoint: Root=0x{:x} (Size: {})",
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
}''',
    "replacement": '''use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::info;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleLeaf {
    pub action: AgentAction,
    pub actor_id: String,
    pub timestamp: u64,
    pub hash: u64,
}
impl MerkleLeaf {
    pub fn new(action: AgentAction, actor_id: &str) -> Self {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut hasher = DefaultHasher::new();
        serde_json::to_vec(&action).unwrap_or_default().hash(&mut hasher);
        actor_id.hash(&mut hasher);
        ts.hash(&mut hasher);
        Self { action, actor_id: actor_id.to_string(), timestamp: ts, hash: hasher.finish() }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GovernanceLog {
    pub history: Vec<MerkleLeaf>,
    pub root_hash: u64,
}
impl GovernanceLog {
    pub fn init() -> Self {
        Self::default()
    }
    pub fn init_shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::init()))
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
        if self.history.len().is_multiple_of(10) {
            info!(
                "[GOVERNANCE] Checkpoint: Root=0x{:x} (Size: {})",
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

// FIX: process-wide persistent log. Previously task_manager.rs called
// GovernanceLog::init() fresh on every directive, so even its one
// record() call was discarded along with the throwaway instance.
// Every real caller should use this static instead of constructing
// their own GovernanceLog.
pub static GOVERNANCE: LazyLock<Mutex<GovernanceLog>> =
    LazyLock::new(|| Mutex::new(GovernanceLog::init()));''',
}

# ─────────────────────────────────────────────────────────────────────
# 2. executor.rs
# ─────────────────────────────────────────────────────────────────────
FILES["crates/runtime_core/src/executor.rs"] = {
    "anchor": '''    pub async fn execute_single_action(&mut self, action: AgentAction, _memory: Arc<Mutex<MemoryPalace>>, _vmo_state: Arc<Mutex<VmoState>>, _dist: Arc<DistributedScheduler>) -> Result<String> {
        match action {''',
    "replacement": '''    pub async fn execute_single_action(&mut self, action: AgentAction, _memory: Arc<Mutex<MemoryPalace>>, _vmo_state: Arc<Mutex<VmoState>>, _dist: Arc<DistributedScheduler>) -> Result<String> {
        let ledger_action = action.clone();
        let result: Result<String> = async {
        match action {''',
}

FILES["crates/runtime_core/src/executor.rs"]["anchor_2"] = '''            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
        }
    }
}

pub struct ToolExecutor;
impl ToolExecutor {
    pub async fn execute(action: &AgentAction) -> Result<String> {
        match action {'''
FILES["crates/runtime_core/src/executor.rs"]["replacement_2"] = '''            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
        }
        }.await;

        if result.is_ok() {
            crate::governance::protocol::GOVERNANCE.lock().await.record(ledger_action, "robotic_executor");
        }

        result
    }
}

pub struct ToolExecutor;
impl ToolExecutor {
    pub async fn execute(action: &AgentAction) -> Result<String> {
        let result: Result<String> = async {
        match action {'''

FILES["crates/runtime_core/src/executor.rs"]["anchor_3"] = '''            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
            other => Ok(format!("[NOT_WIRED] {:?} requires substrate context.", other)),
        }
    }
}'''
FILES["crates/runtime_core/src/executor.rs"]["replacement_3"] = '''            AgentAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
            other => Ok(format!("[NOT_WIRED] {:?} requires substrate context.", other)),
        }
        }.await;

        if result.is_ok() {
            crate::governance::protocol::GOVERNANCE.lock().await.record(action.clone(), "tool_executor");
        }

        result
    }
}'''

# ─────────────────────────────────────────────────────────────────────
# 3. task_manager.rs (full-file replace, small file, safest)
# ─────────────────────────────────────────────────────────────────────
FILES["crates/runtime_core/src/task_manager.rs"] = {
    "anchor": '''#![allow(unused_imports)]
use crate::executor::RoboticExecutor;
use nexus_substrate::memory::MemoryPalace;
use crate::{
    agent_protocol::AgentAction,
    metrics::METRICS,
    orchestrator::Orchestrator,
    shared_state::VmoState,
    distributed_scheduler::DistributedScheduler,
    // v4 systems
    task_manager_v4,
    cortical_loop::CorticalLoop,
    react_agent::ReActAgent,
    governance::protocol::GovernanceLog,
    policy::ExecutionSandbox,
    event_bus::EventBus,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use anyhow::Result;

#[derive(Clone)]
pub struct TaskContext {
    pub client: Client,
    pub api_key: String,
    pub memory: Arc<Mutex<MemoryPalace>>,
    pub vmo_state: Arc<Mutex<VmoState>>,
    pub tx: broadcast::Sender<String>,
    pub dist: Arc<DistributedScheduler>,
}

pub async fn send_task(
    _orchestrator: Arc<Orchestrator>,  // kept for signature compatibility
    ctx: TaskContext,
    raw_directive: &str,
) -> String {
    METRICS.inc_task();

    let _ = ctx.tx.send(serde_json::json!({
        "type": "LOG_EVENT",
        "payload": "[EXECUTIVE CORTEX v4] Starting full reasoning stack..."
    }).to_string());

    // Governance + Sandbox
    let mut governance = GovernanceLog::init();
    let _sandbox = ExecutionSandbox::strict_lockdown();
    let _event_bus = EventBus::new(64);

    // Primary: v4 GoT Path
    match task_manager_v4::send_task_v4(ctx.clone(), raw_directive).await {
        Ok(result) => {
            governance.record(
                AgentAction::Halt { reasoning: "GoT completed".to_string() },
                "nexus_cli"
            );
            let _ = ctx.tx.send(serde_json::json!({
                "type": "LOG_EVENT",
                "payload": "[v4 GoT] Mission accomplished"
            }).to_string());
            return result;
        }
        Err(e) => {
            let _ = ctx.tx.send(serde_json::json!({
                "type": "LOG_EVENT",
                "payload": format!("[v4 fallback] {}", e)
            }).to_string());
        }
    }

    // Secondary: CorticalLoop
    let mut cortical = CorticalLoop::new(
        ctx.api_key.clone(),
        Arc::new(crate::world_model::WorldModel::new()),
        ctx.memory.clone(),
        ctx.vmo_state.clone(),
        ctx.dist.clone(),
        ctx.tx.clone(),
    );

    if let Err(e) = cortical.ignite(raw_directive).await {
        let _ = ctx.tx.send(serde_json::json!({
            "type": "LOG_EVENT",
            "payload": format!("[CORTEX] {}", e)
        }).to_string());
    } else {
        return "[MISSION COMPLETE via CorticalLoop]".to_string();
    }

    format!("[v4] Advanced paths attempted for: {}", raw_directive)
}''',
    "replacement": '''#![allow(unused_imports)]
use crate::executor::RoboticExecutor;
use nexus_substrate::memory::MemoryPalace;
use crate::{
    agent_protocol::AgentAction,
    metrics::METRICS,
    orchestrator::Orchestrator,
    shared_state::VmoState,
    distributed_scheduler::DistributedScheduler,
    // v4 systems
    task_manager_v4,
    cortical_loop::CorticalLoop,
    react_agent::ReActAgent,
    neural_agent::{MicroAgent, ContextPacket},
    governance::protocol::GOVERNANCE,
    policy::ExecutionSandbox,
    event_bus::EventBus,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use anyhow::Result;

#[derive(Clone)]
pub struct TaskContext {
    pub client: Client,
    pub api_key: String,
    pub memory: Arc<Mutex<MemoryPalace>>,
    pub vmo_state: Arc<Mutex<VmoState>>,
    pub tx: broadcast::Sender<String>,
    pub dist: Arc<DistributedScheduler>,
}

// FIX: explicit mode selection replaces the old silent
// try-v4-then-fallback-to-cortical chain. That chain always "succeeded"
// because send_task_v4 discarded its own reasoning output and just
// called the same CorticalLoop::ignite the fallback branch called --
// meaning every directive, regardless of mode, ran the identical code
// path. This also restores the dead ReActAgent as a real option.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionMode {
    GoT,
    ReAct,
    Cortical,
}

pub async fn send_task(
    _orchestrator: Arc<Orchestrator>,  // kept for signature compatibility
    ctx: TaskContext,
    raw_directive: &str,
    mode: ExecutionMode,
) -> String {
    METRICS.inc_task();
    let _sandbox = ExecutionSandbox::strict_lockdown();
    let _event_bus = EventBus::new(64);

    let (result, actor_id): (std::result::Result<String, String>, &str) = match mode {
        ExecutionMode::GoT => {
            let _ = ctx.tx.send(serde_json::json!({
                "type": "LOG_EVENT",
                "payload": "[MODE: v4 GoT] Starting reasoning stack..."
            }).to_string());
            let r = task_manager_v4::send_task_v4(ctx.clone(), raw_directive)
                .await
                .map_err(|e| e.to_string());
            (r, "v4_got")
        }
        ExecutionMode::ReAct => {
            let _ = ctx.tx.send(serde_json::json!({
                "type": "LOG_EVENT",
                "payload": "[MODE: ReAct] Starting ReAct reasoning loop..."
            }).to_string());
            let r = run_react(&ctx, raw_directive).await;
            (r, "react_agent")
        }
        ExecutionMode::Cortical => {
            let _ = ctx.tx.send(serde_json::json!({
                "type": "LOG_EVENT",
                "payload": "[MODE: Cortical] Starting CorticalLoop epistemic engine..."
            }).to_string());
            let r = run_cortical(&ctx, raw_directive).await;
            (r, "cortical_loop")
        }
    };

    match result {
        Ok(output) => {
            GOVERNANCE.lock().await.record(
                AgentAction::Halt { reasoning: format!("{:?} mode completed", mode) },
                actor_id,
            );
            let _ = ctx.tx.send(serde_json::json!({
                "type": "LOG_EVENT",
                "payload": format!("[{}] Mission accomplished", actor_id)
            }).to_string());
            output
        }
        Err(e) => {
            let _ = ctx.tx.send(serde_json::json!({
                "type": "LOG_EVENT",
                "payload": format!("[{}] FAILED: {}", actor_id, e)
            }).to_string());
            format!("[{}] Execution failed: {}", actor_id, e)
        }
    }
}

async fn run_cortical(ctx: &TaskContext, raw_directive: &str) -> std::result::Result<String, String> {
    let mut cortical = CorticalLoop::new(
        ctx.api_key.clone(),
        Arc::new(crate::world_model::WorldModel::new()),
        ctx.memory.clone(),
        ctx.vmo_state.clone(),
        ctx.dist.clone(),
        ctx.tx.clone(),
    );

    cortical.ignite(raw_directive).await
        .map(|_| "[MISSION COMPLETE via CorticalLoop]".to_string())
        .map_err(|e| e.to_string())
}

async fn run_react(ctx: &TaskContext, raw_directive: &str) -> std::result::Result<String, String> {
    let micro_agent = MicroAgent::init(ctx.tx.clone())
        .map(Arc::new)
        .map_err(|e| format!("MICRO_AGENT_INIT_FAULT: {}", e))?;

    let react = ReActAgent::new(
        micro_agent,
        ctx.client.clone(),
        ctx.api_key.clone(),
        ctx.tx.clone(),
    );

    react.run(raw_directive, ContextPacket::default())
        .await
        .map_err(|e| e.to_string())
}''',
}

# ─────────────────────────────────────────────────────────────────────
# 4. bins/cli/src/main.rs
# ─────────────────────────────────────────────────────────────────────
FILES["bins/cli/src/main.rs"] = {
    "anchor": '''use nexus_runtime_core::{
    cluster::ClusterMesh,
    distributed_scheduler::DistributedScheduler,
    orchestrator::Orchestrator,
    shared_state::VmoState,
    task_manager::{send_task, TaskContext},
};''',
    "replacement": '''use nexus_runtime_core::{
    cluster::ClusterMesh,
    distributed_scheduler::DistributedScheduler,
    orchestrator::Orchestrator,
    shared_state::VmoState,
    task_manager::{send_task, TaskContext, ExecutionMode},
};''',
}

FILES["bins/cli/src/main.rs"]["anchor_2"] = '''            "/help" => {
                println!("\\x1b[36mCommands:\\x1b[0m");
                println!("  /status    - View Substrate Load & Active VMOs");
                println!("  /clear     - Purge SQLite Memory Palace");
                println!("  /exit      - Shutdown CLI");
                println!("  <text>     - Submit directive to the Neural Agent");
            }
            "/status" => {
                let active = unsafe { nexus_substrate::kernel::ffi::muscle_get_active_vmos() };
                println!("\\x1b[36m[SUBSTRATE]\\x1b[0m Active VMOs: {}", active);
            }
            "/clear" => {
                let mem = memory.lock().await;
                if let Err(e) = mem.purge_all().await {
                    println!("\\x1b[31m[ERROR]\\x1b[0m Memory purge failed: {}", e);
                } else {
                    println!("\\x1b[32m[MEMORY]\\x1b[0m Neural context eradicated.");
                }
            }
            directive => {
                println!("\\x1b[36m>> Engaging Neural Routing...\\x1b[0m");
                let ctx = TaskContext {
                    client: client.clone(),
                    api_key: api_key.clone(),
                    memory: memory.clone(),
                    vmo_state: vmo_state.clone(),
                    tx: tx.clone(),
                    dist: dist.clone(),
                };
                let response = send_task(
                    orchestrator.clone(),
                    ctx,
                    directive
                ).await;
                println!("\\n\\x1b[0m\\x1b[1;32m[MISSION COMPLETE]\\x1b[0m {}", response);
            }
        }
    }

    Ok(())
}'''
FILES["bins/cli/src/main.rs"]["replacement_2"] = '''            "/help" => {
                println!("\\x1b[36mCommands:\\x1b[0m");
                println!("  /status       - View Substrate Load & Active VMOs");
                println!("  /clear        - Purge SQLite Memory Palace");
                println!("  /exit         - Shutdown CLI");
                println!("  /v4 <text>    - Force v4 GoT reasoning path");
                println!("  /react <text> - Force ReAct reasoning loop");
                println!("  <text>        - Submit directive (default: CorticalLoop)");
            }
            "/status" => {
                let active = unsafe { nexus_substrate::kernel::ffi::muscle_get_active_vmos() };
                println!("\\x1b[36m[SUBSTRATE]\\x1b[0m Active VMOs: {}", active);
            }
            "/clear" => {
                let mem = memory.lock().await;
                if let Err(e) = mem.purge_all().await {
                    println!("\\x1b[31m[ERROR]\\x1b[0m Memory purge failed: {}", e);
                } else {
                    println!("\\x1b[32m[MEMORY]\\x1b[0m Neural context eradicated.");
                }
            }
            cmd if cmd.starts_with("/v4 ") => {
                let directive = cmd["/v4 ".len()..].trim();
                dispatch(directive, ExecutionMode::GoT, &client, &api_key, &memory, &vmo_state, &tx, &dist, &orchestrator).await;
            }
            cmd if cmd.starts_with("/react ") => {
                let directive = cmd["/react ".len()..].trim();
                dispatch(directive, ExecutionMode::ReAct, &client, &api_key, &memory, &vmo_state, &tx, &dist, &orchestrator).await;
            }
            directive => {
                dispatch(directive, ExecutionMode::Cortical, &client, &api_key, &memory, &vmo_state, &tx, &dist, &orchestrator).await;
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn dispatch(
    directive: &str,
    mode: ExecutionMode,
    client: &reqwest::Client,
    api_key: &str,
    memory: &Arc<Mutex<MemoryPalace>>,
    vmo_state: &Arc<Mutex<VmoState>>,
    tx: &broadcast::Sender<String>,
    dist: &Arc<DistributedScheduler>,
    orchestrator: &Arc<Orchestrator>,
) {
    println!("\\x1b[36m>> Engaging Neural Routing ({:?})...\\x1b[0m", mode);
    let ctx = TaskContext {
        client: client.clone(),
        api_key: api_key.to_string(),
        memory: memory.clone(),
        vmo_state: vmo_state.clone(),
        tx: tx.clone(),
        dist: dist.clone(),
    };
    let response = send_task(orchestrator.clone(), ctx, directive, mode).await;
    println!("\\n\\x1b[0m\\x1b[1;32m[MISSION COMPLETE]\\x1b[0m {}", response);
}'''


def check_anchor(path, key, anchor):
    try:
        with open(path, "r", encoding="utf-8") as f:
            content = f.read()
    except FileNotFoundError:
        print("FATAL: " + path + " not found.", file=sys.stderr)
        sys.exit(1)
    count = content.count(anchor)
    if count == 0:
        print("FATAL: anchor '" + key + "' not found in " + path + ". Re-cat and re-derive.", file=sys.stderr)
        sys.exit(1)
    if count > 1:
        print("FATAL: anchor '" + key + "' matched " + str(count) + " times in " + path + ", expected 1.", file=sys.stderr)
        sys.exit(1)
    return content


def main():
    # Phase 1: verify every anchor in every file BEFORE writing anything.
    loaded = {}
    for path, spec in FILES.items():
        content = check_anchor(path, "primary", spec["anchor"])
        for extra_key in ("anchor_2", "anchor_3"):
            if extra_key in spec:
                content = check_anchor(path, extra_key, spec[extra_key])
        loaded[path] = content

    # Phase 2: apply all patches now that every anchor is confirmed present.
    for path, spec in FILES.items():
        content = loaded[path]
        content = content.replace(spec["anchor"], spec["replacement"])
        if "anchor_2" in spec:
            content = content.replace(spec["anchor_2"], spec["replacement_2"])
        if "anchor_3" in spec:
            content = content.replace(spec["anchor_3"], spec["replacement_3"])
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        print("OK: patched " + path)

    print()
    print("All 4 files patched. Next:")
    print("  cargo check -p nexus_runtime_core -p nexus_cli -j 2")


if __name__ == "__main__":
    main()
