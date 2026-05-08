#!/usr/bin/env bash
set -euo pipefail
echo -e "\x1b[35m╔══════════════════════════════════════════════════════════╗\x1b[0m"
echo -e "\x1b[35m║   NEXUS SINGULARITY v4 — AUTONOMOUS RUNTIME FOUNDATION   ║\x1b[0m"
echo -e "\x1b[35m╚══════════════════════════════════════════════════════════╝\x1b[0m"
echo ""

echo "[ 1/9 ] Excising non-critical modules (economy, billing, etc.)..."
rm -rf crates/economy crates/billing crates/marketplace crates/payments crates/governance bins/ide_backend/src/governance
echo "    ✔ Dead code paths eradicated."

echo "[ 2/9 ] Removing excised modules from root Cargo.toml..."
python3 -c '
import os, re
path = "Cargo.toml"
if os.path.exists(path):
    with open(path, "r") as f: data = f.read()
    
    # Remove dead crates from workspace members
    for dead_crate in ["economy", "protocol", "payments", "marketplace", "billing"]:
        data = re.sub(rf"\s*\"crates/{dead_crate}\",?", "", data)
    
    with open(path, "w") as f: f.write(data)
    print("    ✔ Cargo.toml workspace members aligned.")
'

echo "[ 3/9 ] event_bus.rs — Deploying Typed EventRef Bus..."
cat > crates/runtime_core/src/event_bus.rs << 'RUST_EOF'
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    TaskCreated,
    TaskStarted,
    TaskCompleted,
    AgentCrashed,
    ArtifactCommitted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRef {
    pub seq: u64,
    pub kind: EventKind,
    pub object_id: String,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<EventRef>,
}

impl EventBus {
    pub fn new(size: usize) -> Self {
        let (tx, _) = broadcast::channel(size);
        Self { tx }
    }

    pub fn emit(&self, evt: EventRef) {
        let _ = self.tx.send(evt);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventRef> {
        self.tx.subscribe()
    }
}
RUST_EOF
echo "    ✔ event_bus.rs forged."

echo "[ 4/9 ] sqlite_store.rs — Deploying Async WAL Substrate..."
cat > crates/nexus_substrate/src/sqlite_store.rs << 'RUST_EOF'
use anyhow::Result;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(path: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}", path))
            .await?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous=NORMAL;").execute(&pool).await?;

        sqlx::query("
            CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                ts DATETIME DEFAULT CURRENT_TIMESTAMP
            );
        ").execute(&pool).await?;

        sqlx::query("
            CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                payload BLOB NOT NULL
            );
        ").execute(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn append_event(&self, kind: &str, object_id: &str) -> Result<()> {
        sqlx::query("INSERT INTO events (kind, object_id) VALUES (?, ?)")
            .bind(kind)
            .bind(object_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
RUST_EOF
echo "    ✔ sqlite_store.rs forged (sqlx async native)."

echo "[ 5/9 ] memory_actor.rs — Deploying Asynchronous Actor Model..."
cat > crates/runtime_core/src/memory_actor.rs << 'RUST_EOF'
use tokio::sync::{mpsc, oneshot};
use std::collections::HashMap;

pub enum MemoryCommand {
    Store { key: String, value: Vec<u8> },
    Load { key: String, respond_to: oneshot::Sender<Option<Vec<u8>>> },
}

pub struct MemoryActor {
    rx: mpsc::Receiver<MemoryCommand>,
}

impl MemoryActor {
    pub fn new(rx: mpsc::Receiver<MemoryCommand>) -> Self {
        Self { rx }
    }

    pub async fn run(mut self) {
        let mut db = HashMap::new();
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                MemoryCommand::Store { key, value } => { db.insert(key, value); }
                MemoryCommand::Load { key, respond_to } => {
                    let _ = respond_to.send(db.get(&key).cloned());
                }
            }
        }
    }
}
RUST_EOF
echo "    ✔ memory_actor.rs forged."

echo "[ 6/9 ] graph_validator.rs — Deploying Kahn's DAG Validator..."
cat > crates/runtime_core/src/graph_validator.rs << 'RUST_EOF'
use std::collections::{HashMap, VecDeque};
use crate::orchestrator_v2::task_graph::TaskGraph;

pub fn validate_graph(graph: &TaskGraph) -> Result<(), String> {
    let mut indegree = HashMap::new();

    for (id, node) in &graph.nodes {
        indegree.entry(id.clone()).or_insert(0);
        for dep in &node.deps {
            *indegree.entry(dep.clone()).or_insert(0) += 1;
        }
    }

    let mut queue = VecDeque::new();
    for (node, degree) in &indegree {
        if *degree == 0 { queue.push_back(node.clone()); }
    }

    let mut visited = 0;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        if let Some(n) = graph.nodes.get(&node) {
            for dep in &n.deps {
                if let Some(v) = indegree.get_mut(dep) {
                    *v -= 1;
                    if *v == 0 { queue.push_back(dep.clone()); }
                }
            }
        }
    }

    if visited != graph.nodes.len() {
        return Err("GRAPH_CYCLE_DETECTED".into());
    }
    Ok(())
}
RUST_EOF
echo "    ✔ graph_validator.rs forged."

echo "[ 7/9 ] safe_kernel.rs — Deploying Safe FFI Kernel Wrapper..."
cat > crates/nexus_substrate/src/kernel/safe_kernel.rs << 'RUST_EOF'
use crate::kernel::ffi;
use crate::kernel::error::KernelError;

pub struct SafeKernel;

impl SafeKernel {
    pub fn create_vmo(size: usize) -> Result<u32, KernelError> {
        if size == 0 || size > 16_777_216 { return Err(KernelError::InvalidArgs); }
        let handle = unsafe { ffi::zx_vmo_create_native(size) };
        if handle == 0 { return Err(KernelError::InvalidArgs); }
        Ok(handle)
    }

    pub fn close_handle(handle: u32) {
        unsafe { ffi::zx_handle_close_native(handle) }
    }
}
RUST_EOF
echo "    ✔ safe_kernel.rs forged."

echo "[ 8/9 ] wasm_executor.rs — Deploying Fueled Sandbox Wrapper..."
cat > crates/runtime_core/src/wasm_executor.rs << 'RUST_EOF'
use anyhow::Result;
use wasmtime::*;

pub struct WasmExecutor;

impl WasmExecutor {
    pub fn execute(wasm: &[u8], fn_name: &str) -> Result<()> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        
        let module = Module::from_binary(&engine, wasm)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(50_000)?;

        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<(), ()>(&mut store, fn_name)?;
        func.call(&mut store, ())?;

        Ok(())
    }
}
RUST_EOF
echo "    ✔ wasm_executor.rs forged."

echo "[ 9/9 ] runtime_metrics.rs — Deploying Atomic Metric Trackers..."
cat > crates/runtime_core/src/runtime_metrics.rs << 'RUST_EOF'
use std::sync::atomic::{AtomicU64, Ordering};

pub struct RuntimeMetrics {
    pub active_tasks: AtomicU64,
    pub crashed_agents: AtomicU64,
    pub emitted_events: AtomicU64,
}

impl RuntimeMetrics {
    pub fn new() -> Self {
        Self {
            active_tasks: AtomicU64::new(0),
            crashed_agents: AtomicU64::new(0),
            emitted_events: AtomicU64::new(0),
        }
    }

    pub fn inc_tasks(&self) {
        self.active_tasks.fetch_add(1, Ordering::Relaxed);
    }
}
RUST_EOF
echo "    ✔ runtime_metrics.rs forged."

echo ""
echo -e "\x1b[33m>> Executing absolute cache sweep...\x1b[0m"
rm -rf target/
rm -f *.so
echo -e "\x1b[32m>> V4 RUNTIME UPGRADES APPLIED SUCCESSFULLY.\x1b[0m"
