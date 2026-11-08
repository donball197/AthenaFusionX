# Full System Audit

## Scope and Method

This audit covers the `bins/`, `apps/`, and `crates/` directories. Evidence sources are:

- `cargo check --workspace` (exit status 0; `nexus_ide_backend` generated 26 warnings).
- Repository-wide searches for `todo!` and `unimplemented!`.
- Direct reads of `bins/ide_backend/src/main.rs` and `crates/runtime_core/src/task_manager.rs`.

## 1. Orphaned Files and Dead Code Found via Cargo Warnings

`cargo check --workspace` completed successfully with exit status 0. The captured warning output was truncated, but it explicitly reports that `nexus_ide_backend` generated 26 warnings. The following dead-code locations are directly evidenced in the captured output:

- `bins/ide_backend/src/telemetry_bridge.rs:32` â `pub fn init()` is never used.
- `bins/ide_backend/src/thermal_governor.rs:8` â `pub fn init()` is never used.
- `bins/ide_backend/src/thermal_governor.rs:12` â `pub fn is_throttled()` is never used.
- `bins/ide_backend/src/thermal_governor.rs:15` â `pub fn is_paused()` is never used.
- `bins/ide_backend/src/thermal_governor.rs:18` â `pub fn current_temp()` is never used.

Additional orphaned/backup artifacts are present in the source tree and should be treated as dead code or accidental repository pollution:

- `crates/runtime_core/src/orchestrator_v2/planner.rs.bak`
- `crates/runtime_core/src/orchestrator_v2/planner.rs.bak.1793921733`
- `crates/runtime_core/src/cortical/halt_claims.rs.pre_fix.bak`

These backup files duplicate active source and are not part of the normal compiled crate surface. They should be removed from version control or moved outside the source tree.

Because the full `cargo check` stderr was truncated in the available evidence, the complete 26-warning list should be regenerated with:

```sh
cargo check --workspace 2>&1 | tee /tmp/cargo_check_warnings.txt
```

## 2. Exact Locations of Stubs, Placeholders, and Technical Debt

### 2.1 `todo!` and `unimplemented!` macros

No production `todo!` or `unimplemented!` macros were found in `bins/`, `apps/`, or `crates/`. All matches are test fixtures, audit-contract strings, or documentation embedded in source/tests, not runtime stubs. Representative exact locations include:

- `crates/runtime_core/src/orchestrator_v2/verifier.rs:296-324` â test strings containing `todo!` search markers.
- `crates/runtime_core/src/executor.rs:1237-1241` â test validating `unimplemented!` as a search query.
- `crates/runtime_core/src/completion_engine.rs:709-713` â comments about collapsed `unimplemented!` search targets.
- `crates/runtime_core/src/completion_engine.rs:2258-2269` â test contract using `bins:unimplemented!`.
- `crates/runtime_core/src/evidence_markers.rs:400-427` and nearby lines around `581+` â tests distinguishing crates vs. bins `unimplemented!` markers.
- `crates/runtime_core/src/cortical/halt_claims.rs:48-52` and `1001-1005` â audit term lists and embedded root directive text.
- `crates/runtime_core/src/orchestrator_v2/planner.rs:290-294` and `1546-1803` â audit-search rules and tests.

Conclusion: there are no direct runtime stubs using `todo!` or `unimplemented!`. The remaining technical debt is structural rather than macro-based.

### 2.2 Technical debt in `bins/ide_backend/src/main.rs`

- `acquire_daemon_lock()` uses a relative lock path, `.nexus_daemon.lock`, which makes daemon ownership dependent on the current working directory.
- `MemoryPalace::recover_or_abort_orphaned_plans()` failure is logged with `tracing::error!` but not propagated; startup continues after recovery failure.
- Background subsystems are spawned without retained `JoinHandle`s:
  - `dream_cycle::spawn(...)`
  - `self_healing_engine::spawn(...)`
  - `telemetry_bridge::spawn(...)`
  - `thermal_governor::spawn()`
  - `heartbeat::spawn(...)`
- WASM sidecar execution failures are logged but not propagated or retried.
- Sidecar panics are logged by the `JoinSet` watcher, but the failed sidecar is not restarted.
- SIGTERM/SIGINT handling calls `abort_plans_owned_by(...)` and then `std::process::exit(0)`, bypassing graceful Axum/server shutdown and any in-flight task cleanup.
- `telemetry_bridge` and `thermal_governor` expose unused public functions, indicating partially wired subsystems.

### 2.3 Technical debt in `crates/runtime_core/src/task_manager.rs`

- `send_task` converts all `TaskError` variants to strings at the public boundary, discarding structured error type information.
- `PlanGuard::drop` attempts cleanup by spawning an async abort through `tokio::runtime::Handle::try_current()`; if no runtime is available, cleanup is silently skipped.
- `cortex_handle.is_finished()` detects unexpected Executive Cortex termination, but it does not detect a hung or stalled cortex.
- Dependency-blocked nodes set `TaskStatus::Failed` and `continue` before `ctx.memory.update_plan_run_checkpoint(...)`, so their failed state is not checkpointed at that point.
- Time values use `std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()`, which silently substitutes zero on clock errors.
- `uuid::Uuid::parse_str(&task_id).unwrap_or_else(|_| uuid::Uuid::new_v4())` silently substitutes a new UUID when parsing fails.
- The whole-DAG completion gate requires `completed_node_count == planned_node_count`; this is correct, but blocked-node checkpoint gaps reduce crash-recovery fidelity.

## 3. Duplicated Logic Across Crates

The strongest duplication is in the audit/search verification harness rather than in core business logic:

- `crates/runtime_core/src/orchestrator_v2/planner.rs`
- `crates/runtime_core/src/orchestrator_v2/verifier.rs`
- `crates/runtime_core/src/evidence_markers.rs`
- `crates/runtime_core/src/completion_engine.rs`
- `crates/runtime_core/src/cortical/halt_claims.rs`

These files repeatedly encode the same search-marker concepts, such as `SEARCH_CODE_OK`, `SEARCH_CHECKED_OK`, `bins:todo!`, `crates:todo!`, `bins:unimplemented!`, and `crates:unimplemented!`. This duplication increases the risk that one audit path accepts a marker while another rejects it.

Additional duplication:

- Backup copies of `planner.rs` and `halt_claims.rs` duplicate active source and can drift from the compiled implementation.
- `telemetry_bridge::init` and `thermal_governor::init` follow the same unused initialization pattern, suggesting duplicated startup boilerplate that is not actually consumed by `main.rs`.

No duplicated production DAG execution or persistence logic was evidenced beyond the shared audit/search verification patterns above.

## 4. Architectural Advice for Live, Unmonitored Production Runs

### 4.1 Panic recovery

- Wrap sidecar execution in `std::panic::catch_unwind` and place it under a supervisor with bounded restart/backoff. The current `JoinSet` only logs panics and leaves the sidecar permanently unavailable.
- Retain `JoinHandle`s for all background tasks and monitor them with a watchdog. Restart failed background tasks instead of allowing silent subsystem loss.
- Replace `PlanGuard` drop-based cleanup with explicit cancellation scopes or RAII guards that do not depend on `tokio::runtime::Handle::try_current()`.
- Make the signal handler perform graceful shutdown: stop accepting work, cancel in-flight tasks with a timeout, flush checkpoints, then exit. Avoid `std::process::exit(0)` as the normal shutdown path.
- Add cortex liveness heartbeats and timeouts. `is_finished()` alone cannot detect a stalled actor.

### 4.2 State durability

- Checkpoint dependency-blocked nodes as well, or persist their `Failed` status before `continue`. Crash recovery should reflect the exact DAG state.
- Make checkpoint writes transactional and durable. Use SQLite WAL mode, busy timeouts, and fsync where appropriate.
- Persist task outputs and evidence alongside graph status so recovery can resume from the last verified node rather than aborting all orphaned plans.
- Distinguish recoverable, stale-heartbeat, and live-owner plans with explicit fencing tokens and owner identities. Do not preserve stale-heartbeat plans indefinitely without a timeout policy.
- Use absolute, configuration-driven paths for `.nexus_daemon.lock`, `nexus_daemon.db`, and `sidecars`.
- Preserve structured `TaskError` variants across the `send_task` boundary for diagnostics and automated recovery decisions.

### 4.3 Scaling

- Wire the existing `ClusterMesh` and `DistributedScheduler` into actual DAG dispatch, or remove them if the system remains single-node. Dead distributed abstractions create false scaling assumptions.
- Add bounded concurrency controls: per-node semaphores, global worker limits, and backpressure on the broadcast channel.
- Use connection pooling and WAL for SQLite, or migrate plan state to a production-grade database if multi-node execution is required.
- Run WASM sidecars in a managed pool with CPU/memory limits and restart policies.
- Integrate or remove `telemetry_bridge` and `thermal_governor`; unused monitoring code should not remain in the production binary.
- Export metrics, traces, and health endpoints. Unmonitored production runs require observable liveness, checkpoint latency, task failure rates, and sidecar health.
- Avoid relative filesystem assumptions and blocking filesystem operations on the async runtime where possible.

## 5. Summary

The workspace compiles successfully, but the daemon has significant production-readiness gaps: 26 warnings in `nexus_ide_backend`, unused telemetry/thermal modules, backup source files, checkpoint gaps for blocked DAG nodes, string-flattened errors, non-graceful shutdown, and unmanaged background/sidecar tasks. No `todo!` or `unimplemented!` runtime stubs were found. The most important next improvements are deterministic checkpointing of every DAG state transition, graceful panic-aware supervision, durable resumable plan state, and real bounded concurrency for scaling.
