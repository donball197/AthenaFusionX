# Roadmap: OpenFang vs. Project Nexus Singularity v1.9

## 1. Crate Topology Mapping
The following table maps the OpenFang workspace structure to the Nexus Singularity core components to facilitate assimilation of logic and patterns.

| OpenFang Crate | Nexus Component | Primary Responsibility | Synthesis Note |
| :--- | :--- | :--- | :--- |
| `openfang-kernel` | `nexus_runtime_core` | Central Coordination & Orchestration | Map `OpenFangKernel` logic to Nexus Supervisor/Scheduler. |
| `openfang-runtime` | `nexus_runtime_core` | Execution Engine & Sandbox | Assimilate dual-metering logic into VmoHandler. |
| `openfang-memory` | `nexus_substrate` | Persistence & Vector State | Compare SQLite WAL patterns with `SqliteCheckpointer`. |
| `openfang-types` | `nexus_common` / `nexus_substrate` | Shared Type Definitions | Port `TaintLabel` and `AgentManifest` schemas. |
| `openfang-api` | `nexus_api_gateway` | External Interface (Axum) | Align OpenAI-compatible endpoints with Nexus API. |

## 2. Sandbox Mechanics Breakdown
OpenFang utilizes a software-defined WASM sandbox, whereas Nexus employs a lower-level hardware-assisted/virtualized approach.

| Feature | OpenFang (Wasmtime) | Project Nexus (VmoHandler/ARM64 DNA) |
| :--- | :--- | :--- |
| **Primary Limit** | **Fuel Metering**: Instruction-count based termination. | **Hardware Traps**: Memory protection and CPU cycle limits. |
| **Hanging Guard** | **Epoch Watchdog**: Separate thread kills runaway code. | **VmoHandler Interrupts**: Direct control over execution slices. |
| **Isolation** | Software-defined (WASM VM). | Hardware-assisted (ARM64 DNA/Virtualization). |
| **Overhead** | Moderate (JIT/Interpretation). | Low (Near-native execution). |

**Synthesis Goal:** Implement the "Epoch Watchdog" concept within the `VmoHandler` to provide a secondary safety net for tasks that are fuel-efficient but logically infinite.

## 3. Persistence & Vector Architecture
Both systems leverage SQLite, but their concurrency and recall strategies differ.

| Component | OpenFang (`openfang-memory`) | Project Nexus (`nexus_substrate`) |
| :--- | :--- | :--- |
| **Concurrency** | `Arc<Mutex<Connection>>` + `spawn_blocking`. | `SqliteCheckpointer` + MemoryPalace. |
| **Write Mode** | SQLite WAL (Write-Ahead Logging) for concurrency. | Optimized Checkpointing for state snapshots. |
| **Vector Recall** | Integrated semantic search in SQLite substrate. | Native `fastembed` vector recall in `nexus_substrate`. |
| **Schema** | Versioned (V1 $\rightarrow$ V5) with canonical sessions. | Dynamic substrate with MemoryPalace indexing. |

**Synthesis Goal:** Adopt OpenFang's "Canonical Session" model to improve cross-channel memory consistency in `nexus_substrate`.

## 4. Rust Guardrails for Nexus Singularity v1.9
The following patterns are identified for immediate assimilation into the Nexus codebase to enhance security and stability.

### Pattern A: The Dual-Metered Watchdog (Safety)
Prevents "slow-burn" infinite loops that bypass fuel limits by using a temporal watchdog.

rust
pub struct ExecutionWatchdog {
    timeout: std::time::Duration,
    stop_signal: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ExecutionWatchdog {
    pub async fn monitor<F, R>(&self, mut execute: F) -> Result<R, SandboxError> 
    where F: FnMut() -> Result<R, SandboxError> + Send + 'static, R: Send + 'static 
    {
        let stop = self.stop_signal.clone();
        let timeout = self.timeout;
        
        let watchdog_handle = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        let result = execute();
        watchdog_handle.abort();
        result
    }
}


### Pattern B: Taint-Aware Data Flow (Security)
Prevents leakage of privileged data into untrusted sinks using a wrapper type.

rust
pub struct Tainted<T> {
    pub value: T,
    pub label: TaintLabel, // e.g., Secret, Internal, Public
}

impl<T> Tainted<T> {
    pub fn flow_to(self, sink_level: TaintLabel) -> Result<T, SecurityError> {
        if self.label > sink_level {
            return Err(SecurityError::TaintViolation("Privileged data leaked to untrusted sink"));
        }
        Ok(self.value)
    }
}


### Pattern C: Async-to-Blocking SQLite Bridge (Performance)
Ensures synchronous SQLite operations do not starve the Tokio executor.

rust
pub async fn db_exec<F, R>(conn: std::sync::Arc<tokio::sync::Mutex<rusqlite::Connection>>, f: F) -> Result<R, DbError>
where 
    F: FnOnce(&rusqlite::Connection) -> Result<R, rusqlite::Error> + Send + 'static,
    R: Send + 'static 
{
    let lock = conn.lock().await;
    // Use spawn_blocking to move the synchronous rusqlite call off the async executor
    tokio::task::spawn_blocking(move || {
        f(&lock).map_err(DbError::from)
    }).await.map_err(|_| DbError::JoinError)?
}
