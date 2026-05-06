#!/usr/bin/env bash
set -e

echo -e "\x1b[35m>> [NEXUS V3.0] INJECTING UPGRADED DAG ORCHESTRATOR & SAFE VMO...\x1b[0m"

# ---------------------------------------------------------
# 1. REBUILD KERNEL SUBSTRATE MODULE (For Safe VMO)
# ---------------------------------------------------------
mkdir -p crates/nexus_substrate/src/kernel

cat << 'EOF' > crates/nexus_substrate/src/kernel/error.rs
#[derive(Debug)]
pub enum KernelError {
    VmoCloneFailed { parent: u32, size: usize },
    InvalidArgs,
    RegistryFull,
    AlignmentAnomaly { address: usize },
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for KernelError {}
EOF

cat << 'EOF' > crates/nexus_substrate/src/kernel/mod.rs
pub mod ffi;
pub mod error;
pub mod safe_vmo;

use std::collections::HashMap;
use parking_lot::RwLock;
use once_cell::sync::Lazy;

pub enum HandleType {
    Vmo(u32),
}

pub static HANDLE_REGISTRY: Lazy<RwLock<HashMap<u32, HandleType>>> = 
    Lazy::new(|| RwLock::new(HashMap::new()));

pub fn register_handle(id: u32, h: HandleType) {
    HANDLE_REGISTRY.write().insert(id, h);
}
EOF

# Update nexus_substrate lib.rs to use the new folder-based kernel module
cat << 'EOF' > crates/nexus_substrate/src/lib.rs
pub mod kernel;
pub mod memory;
pub mod vmo_handler;
pub mod vector_bridge;
pub mod token_ledger;
EOF

# ---------------------------------------------------------
# 2. ZERO-ALLOCATION SAFE VMO
# ---------------------------------------------------------
cat << 'EOF' > crates/nexus_substrate/src/kernel/safe_vmo.rs
use crate::kernel::{register_handle, HandleType};
use crate::kernel::ffi;
use crate::kernel::error::KernelError;

pub struct SafeVmo {
    handle: u32,
    pub size: usize,
}

impl SafeVmo {
    pub fn new(size: usize) -> Result<Self, KernelError> {
        let handle = unsafe { ffi::zx_vmo_create_native(size) };
        if handle == 0 {
            return Err(KernelError::InvalidArgs);
        }
        register_handle(handle, HandleType::Vmo(handle));
        Ok(Self { handle, size })
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> Result<(), KernelError> {
        if offset + data.len() > self.size {
            return Err(KernelError::InvalidArgs);
        }
        let status = unsafe {
            ffi::zx_vmo_write_native(self.handle, offset, data.as_ptr(), data.len())
        };
        if status == 0 { Ok(()) } else { Err(KernelError::InvalidArgs) }
    }

    /// ⚡ ZERO-ALLOCATION READ FIX: Prevents heap fragmentation on the 4GB Duet
    pub fn read_into(&self, offset: usize, buffer: &mut [u8]) -> Result<(), KernelError> {
        if offset + buffer.len() > self.size {
            return Err(KernelError::InvalidArgs);
        }
        let status = unsafe {
            ffi::zx_vmo_read_native(self.handle, offset, buffer.as_mut_ptr(), buffer.len())
        };
        if status == 0 { Ok(()) } else { Err(KernelError::InvalidArgs) }
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for SafeVmo {
    fn drop(&mut self) {
        unsafe { ffi::zx_handle_close_native(self.handle) };
    }
}
EOF

# ---------------------------------------------------------
# 3. CAPABILITY-BASED AGENT SANDBOX
# ---------------------------------------------------------
mkdir -p crates/runtime_core/src/agent_runtime

cat << 'EOF' > crates/runtime_core/src/agent_runtime/sandbox.rs
use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Capabilities: u32 {
        const NONE     = 0;
        const READ     = 1 << 0;
        const WRITE    = 1 << 1;
        const EXEC     = 1 << 2;
        const NETWORK  = 1 << 3;
    }
}

pub struct Sandbox {
    caps: Capabilities,
}

impl Sandbox {
    pub fn new(caps: Capabilities) -> Self { Self { caps } }
    pub fn can_read(&self) -> bool { self.caps.contains(Capabilities::READ) }
    pub fn can_write(&self) -> bool { self.caps.contains(Capabilities::WRITE) }
    pub fn can_exec(&self) -> bool { self.caps.contains(Capabilities::EXEC) }
    pub fn can_network(&self) -> bool { self.caps.contains(Capabilities::NETWORK) }
}
EOF

# ---------------------------------------------------------
# 4. REAL AGENT EXECUTION ENGINE
# ---------------------------------------------------------
cat << 'EOF' > crates/runtime_core/src/agent_runtime/engine.rs
use anyhow::Result;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    pub output: String,
    pub success: bool,
}

#[async_trait]
pub trait ExecutableAgent: Send + Sync {
    async fn execute(&self, task: Task) -> Result<TaskResult>;
}
EOF

cat << 'EOF' > crates/runtime_core/src/agent_runtime/mod.rs
pub mod sandbox;
pub mod engine;
EOF

# ---------------------------------------------------------
# 5. TASK GRAPH (DEADLOCK IMMUNE)
# ---------------------------------------------------------
mkdir -p crates/runtime_core/src/orchestrator_v2

cat << 'EOF' > crates/runtime_core/src/orchestrator_v2/task_graph.rs
use std::collections::{HashMap, VecDeque};
use anyhow::{anyhow, Result};

#[derive(Clone)]
pub struct TaskNode {
    pub id: String,
    pub deps: Vec<String>,
    pub required_agent: String,
}

pub struct TaskGraph {
    pub nodes: HashMap<String, TaskNode>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    pub fn add_task(&mut self, id: &str, deps: Vec<String>, required_agent: &str) {
        self.nodes.insert(id.to_string(), TaskNode {
            id: id.to_string(),
            deps,
            required_agent: required_agent.to_string(),
        });
    }

    pub fn resolve_execution_order(&self) -> Result<Vec<String>> {
        let mut resolved = Vec::new();
        let mut queue: VecDeque<_> = self.nodes.values().cloned().collect();
        
        let mut iterations = 0;
        let max_iterations = self.nodes.len() * self.nodes.len() + 1;

        while let Some(node) = queue.pop_front() {
            iterations += 1;
            
            // ⚡ DEADLOCK TRAP FIXED
            if iterations > max_iterations {
                return Err(anyhow!("DAG_DEADLOCK: Circular dependency detected. Aborting."));
            }

            if node.deps.iter().all(|d| resolved.contains(d)) {
                resolved.push(node.id.clone());
            } else {
                queue.push_back(node);
            }
        }
        Ok(resolved)
    }
}
EOF

# ---------------------------------------------------------
# 6. HETEROGENEOUS ORCHESTRATOR V2
# ---------------------------------------------------------
cat << 'EOF' > crates/runtime_core/src/orchestrator_v2/mod.rs
pub mod task_graph;

use crate::agent_runtime::engine::{Task, TaskResult, ExecutableAgent};
use self::task_graph::TaskGraph;
use std::collections::HashMap;
use anyhow::{anyhow, Result};

pub struct OrchestratorV2 {
    // ⚡ DYNAMIC DISPATCH: Supports a diverse swarm of agents simultaneously
    agents: HashMap<String, Box<dyn ExecutableAgent>>, 
}

impl OrchestratorV2 {
    pub fn new() -> Self {
        Self { agents: HashMap::new() }
    }

    pub fn register_agent(&mut self, name: &str, agent: Box<dyn ExecutableAgent>) {
        self.agents.insert(name.to_string(), agent);
    }

    pub async fn execute_graph(&self, graph: TaskGraph) -> Result<Vec<TaskResult>> {
        let order = graph.resolve_execution_order()?;
        let mut results = Vec::new();

        for id in order {
            let node = graph.nodes.get(&id).unwrap();
            
            let agent = self.agents.get(&node.required_agent)
                .ok_or_else(|| anyhow!("AGENT_MISSING: '{}' not registered in swarm.", node.required_agent))?;

            let task = Task {
                id: id.clone(),
                payload: format!("Execute directive for {}", id),
            };

            let result = agent.execute(task).await?;
            results.push(result);
        }

        Ok(results)
    }
}
EOF

# ---------------------------------------------------------
# 7. EXAMPLE REAL WORKER AGENT
# ---------------------------------------------------------
cat << 'EOF' > crates/runtime_core/src/agents/v2_worker.rs
use anyhow::Result;
use async_trait::async_trait;
use crate::agent_runtime::engine::{ExecutableAgent, Task, TaskResult};

pub struct WorkerAgent;

#[async_trait]
impl ExecutableAgent for WorkerAgent {
    async fn execute(&self, task: Task) -> Result<TaskResult> {
        let output = format!("Processed task via Native Rust Traits: {}", task.payload);
        Ok(TaskResult {
            id: task.id,
            output,
            success: true,
        })
    }
}
EOF

# ---------------------------------------------------------
# 8. THE NEW CLI (WIRED TO THE V2 ENGINE)
# ---------------------------------------------------------
cat << 'EOF' > bins/cli/src/main.rs
use nexus_runtime_core::agents::v2_worker::WorkerAgent;
use nexus_runtime_core::orchestrator_v2::OrchestratorV2;
use nexus_runtime_core::orchestrator_v2::task_graph::TaskGraph;

#[tokio::main]
async fn main() {
    println!("\x1b[36m>> [NEXUS] IGNITING V2 DAG ORCHESTRATOR...\x1b[0m");
    
    let mut orchestrator = OrchestratorV2::new();
    orchestrator.register_agent("worker", Box::new(WorkerAgent));

    let mut graph = TaskGraph::new();
    graph.add_task("compile_wasm", vec![], "worker");
    graph.add_task("run_sandbox", vec!["compile_wasm".into()], "worker");
    graph.add_task("extract_metrics", vec!["run_sandbox".into()], "worker");

    match orchestrator.execute_graph(graph).await {
        Ok(results) => {
            for r in results {
                println!("\x1b[32m[{}]\x1b[0m {}", r.id, r.output);
            }
        }
        Err(e) => {
            eprintln!("\x1b[31m[CRITICAL FAULT] {}\x1b[0m", e);
        }
    }
}
EOF

# ---------------------------------------------------------
# 9. WIRE THE MODULE TREE AND DEPENDENCIES
# ---------------------------------------------------------
grep -q "pub mod agent_runtime;" crates/runtime_core/src/lib.rs || echo "pub mod agent_runtime;" >> crates/runtime_core/src/lib.rs
grep -q "pub mod orchestrator_v2;" crates/runtime_core/src/lib.rs || echo "pub mod orchestrator_v2;" >> crates/runtime_core/src/lib.rs
grep -q "pub mod v2_worker;" crates/runtime_core/src/agents/mod.rs || echo "pub mod v2_worker;" >> crates/runtime_core/src/agents/mod.rs

grep -q "bitflags" crates/runtime_core/Cargo.toml || echo 'bitflags = "2.4"' >> crates/runtime_core/Cargo.toml
grep -q "async-trait" crates/runtime_core/Cargo.toml || echo 'async-trait = "0.1"' >> crates/runtime_core/Cargo.toml

# Parking lot is used by the safe VMO registry
grep -q "parking_lot" crates/nexus_substrate/Cargo.toml || echo 'parking_lot = "0.12"' >> crates/nexus_substrate/Cargo.toml

echo -e "\x1b[35m>> [VERIFICATION] Compiling federated core with v2 DAG engine...\x1b[0m"
cargo check -p nexus_runtime_core -p nexus_cli -p nexus_substrate
