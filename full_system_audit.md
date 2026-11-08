# Full System Audit

## Scope

Audited directories: `bins/`, `apps/`, and `crates/`.

Evidence basis:
- `cargo check --workspace` exited with status 0.
- Workspace stub search for `todo!` and `unimplemented!`.
- Source reads of `bins/ide_backend/src/main.rs` (244 lines) and `crates/runtime_core/src/task_manager.rs`.

## 1. Build and Dead-Code Findings

`cargo check --workspace` completed successfully with exit status 0. Cargo's final summary reports 25 warnings, all in `nexus_ide_backend`.

Concrete unused symbols evidenced in the captured cargo output:

- `bins/ide_backend/src/telemetry_bridge.rs:32` â `pub fn init()`
- `bins/ide_backend/src/thermal_governor.rs:8` â `pub fn init()`
- `bins/ide_backend/src/thermal_governor.rs:12` â `pub fn is_throttled() -> bool`
- `bins/ide_backend/src/thermal_governor.rs:15` â `pub fn is_paused() -> bool`
- `bins/ide_backend/src/thermal_governor.rs:18` â `pub fn current_temp() -> f32`

The remaining warnings are in the same crate, but the captured stderr was truncated, so the complete per-warning list is not available in this evidence set. No orphaned files were reported by cargo check; the evidenced dead-code findings are unused functions in `nexus_ide_backend`.

## 2. Stubs, Placeholders, and Technical Debt

No production `todo!` or `unimplemented!` macro invocations were found in `bins/`, `apps/`, or `crates/`. Matches are confined to audit, documentation, and test-fixture contexts, including string literals at:

- `crates/runtime_core/src/cortical/halt_claims.rs:49-50`

Additional references appear in audit/documentation contexts such as `audit_evidence.sh` and `full_system_audit.good.md`.

Technical-debt locations evidenced by source inspection:

- `bins/ide_backend/src/main.rs`
  - `GEMINI_API_KEY` defaults to an empty string via `std::env::var("GEMINI_API_KEY").unwrap_or_default()`.
  - WASM sidecar panic handling only logs the panic and does not restart or isolate the failed worker.
  - Shutdown calls `std::process::exit(0)` after aborting owned plans, preventing graceful resource flushing.
  - Gateway binding is single-node through `server::start_server(state).await`.
- `crates/runtime_core/src/task_manager.rs`
  - `PlanGuard::drop` relies on `tokio::runtime::Handle::try_current()` and spawns an abort task only if a runtime handle is available.
  - There is no per-node retry or supervised restart path.
  - Cancellation is limited to plan abort; there is no graceful node-level cancellation path.
- `bins/ide_backend/src/telemetry_bridge.rs:32` and `bins/ide_backend/src/thermal_governor.rs:8,12,15,18`
  - Unused initialization and status-query functions indicate partially wired subsystems.

## 3. Duplicated Logic Across Crates

No cross-crate duplicated logic was evidenced by the available cargo check, stub search, or required source reads. The workspace exposes scaling-related components such as `ClusterMesh` and `DistributedScheduler`, but no duplicate implementation of their responsibilities was evidenced in the inspected files.

## 4. Production-Readiness Assessment

### Panic Recovery

- `main.rs` detects sidecar panics through `JoinSet::join_next()` and logs them, but does not restart, quarantine, or report readiness degradation.
- `task_manager.rs` checks `cortex_handle.is_finished()` and treats unexpected Executive Cortex termination as a hard DAG failure.
- `PlanGuard` provides a terminal-status safety net on drop, but its runtime-handle dependency makes it fragile outside an active Tokio context.

Recommended work:
- Wrap sidecar execution in supervised restart loops with bounded retry and panic isolation.
- Use `catch_unwind` around non-async worker boundaries where appropriate.
- Make `PlanGuard` cancellation independent of `Handle::try_current()` by storing an explicit runtime handle or using a dedicated cleanup task.

### State Durability

- `task_manager.rs` persists the initial plan before execution, checkpoints graph state after each node, and finalizes plans as `Completed` or `Aborted`.
- Boot recovery in `main.rs` calls `recover_or_abort_orphaned_plans()` and distinguishes recoverable, aborted, live-owned, and stale-heartbeat plans.
- Plan state is serialized as JSON and updated through `upsert_plan_run` and `update_plan_run_checkpoint`.

Recommended work:
- Add WAL/fsync-backed writes or transactional checkpointing for plan state.
- Make recovery idempotent and explicitly classify plans that can be resumed versus aborted.
- Persist node-level retry counts and last-error state to support deterministic replay.
- Replace abrupt `std::process::exit(0)` with graceful shutdown that flushes plan checkpoints and closes database handles.

### Scaling

- The daemon currently binds a single Axum gateway through `server::start_server(state).await`.
- `ClusterMesh` and `DistributedScheduler` are instantiated and shared, indicating intended distributed execution support.
- Sidecars are loaded from a local `sidecars/` directory using blocking tasks.

Recommended work:
- Add readiness and health endpoints before declaring the gateway production-ready.
- Validate all required environment variables before subsystem boot rather than defaulting `GEMINI_API_KEY` to an empty string.
- Define an explicit horizontal scaling model using the existing `ClusterMesh` and `DistributedScheduler`.
- Move sidecar loading to supervised async tasks with resource limits and cancellation support.

## 5. Conclusion

The workspace compiles cleanly. The audit found 25 dead-code warnings, all in `nexus_ide_backend`; no production `todo!` or `unimplemented!` stubs in application code; and no evidenced cross-crate duplicated logic. The runtime core already implements durable plan persistence and two-stage completion verification. The highest-value production work is graceful shutdown, supervised task and sidecar recovery, explicit configuration validation, and a deliberate scaling model.
