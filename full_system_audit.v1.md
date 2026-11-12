# AthenaFusionX Full System Audit

## Scope & Method

This audit is based on reading exactly five files in a single node:
- `bins/ide_backend/src/main.rs`
- `bins/ide_backend/src/server.rs`
- `crates/runtime_core/src/task_manager.rs`
- `crates/nexus_substrate/src/kernel/temp_vmo.rs`
- `crates/nexus_edge_minion/src/decoder_layer.rs`

MemoryPalace, CorticalLoop, the verifier, and most crates were not traced in this node. Findings are limited to what was physically read above.

## Findings

### 1. `supervise_worker` defined but never called (R-5 regression guard)
- **File:** `bins/ide_backend/src/main.rs`, lines 1–24 (definition), lines 270–283 (regression guard test).
- **Label:** CONFIRMED
- **Detail:** `supervise_worker` is defined at the top of `main.rs` but the background subsystems (`dream_cycle::spawn`, `self_healing_engine::spawn`, `telemetry_bridge::spawn`, `thermal_governor::spawn`, `heartbeat::spawn`) are spawned directly without supervision. The regression test `supervise_worker_is_not_dead_code` asserts `call_count > def_count`, which currently fails because `supervise_worker(` appears only in its own definition. This is a confirmed regression: background workers are not supervised with exponential backoff.

### 2. Daemon lock and auth key fail-fast
- **File:** `bins/ide_backend/src/main.rs`, lines 42–60 (`acquire_daemon_lock`), lines 63–64 (`auth::load_key_at_startup`).
- **Label:** CONFIRMED
- **Detail:** Exclusive flock on `.nexus_daemon.lock` is acquired before token creation/DB init; auth key is loaded fail-fast. This is a sound startup ordering.

### 3. `TempVmoFs::read_file` is a placeholder
- **File:** `crates/nexus_substrate/src/kernel/temp_vmo.rs`, lines 12–15.
- **Label:** CONFIRMED
- **Detail:** `read_file` returns a hardcoded TODO byte string instead of reading from a real VMO layer. Any consumer relying on this for file reads gets placeholder content.

### 4. Decoder layer attention is unimplemented
- **File:** `crates/nexus_edge_minion/src/decoder_layer.rs`, lines 1–10 (module doc), lines 118–122 (TODO).
- **Label:** CONFIRMED
- **Detail:** The QK^T/softmax/*V attention score computation is not implemented. `compute_attention_output` returns a placeholder zero vector. The attention branch contributes nothing to the hidden state. This is a known roadmap item, not a hidden defect, but it means the decoder layer cannot produce meaningful attention output.

### 5. `DecoderLayerWeights` KV-shared layer handling
- **File:** `crates/nexus_edge_minion/src/decoder_layer.rs`, lines 18–46.
- **Label:** LIKELY
- **Detail:** `k_proj`, `v_proj`, `k_norm` are `Option<>` because it is unverified whether KV-shared layers carry their own copies. Callers must pass `None` for shared layers. This is a design uncertainty that could lead to panics if `is_kv_source=true` is set without providing the required projections (see `expect` calls at lines 108–110).

### 6. `unified_gateway` spawns task without awaiting
- **File:** `bins/ide_backend/src/server.rs`, lines 44–66 (`unified_gateway`).
- **Label:** CONFIRMED
- **Detail:** The handler spawns a `tokio::spawn` and immediately returns `QUEUED`. There is no handle tracking or error propagation to the caller. If the spawned task panics, the client never learns.

### 7. `safe_path` traversal guard
- **File:** `bins/ide_backend/src/server.rs`, lines 78–90 (`safe_path`).
- **Label:** CONFIRMED
- **Detail:** Rejects absolute paths and `ParentDir` components. This is a sound path-traversal guard for the FS handlers (`fs_read_handler`, `fs_write_handler`, `fs_list_handler`).

### 8. Task DAG execution with two-stage verification
- **File:** `crates/runtime_core/src/task_manager.rs`, lines 100–400+ (`send_task_inner`).
- **Label:** CONFIRMED
- **Detail:** Each DAG node requires both Cortex `CompletionProposed` and deterministic `Verifier::verify_contract` before being marked Completed. `PlanGuard` (lines 60–88) ensures terminal status on drop. Whole-DAG completion requires all planned nodes to complete. This is a robust two-stage gate design.

### 9. `PlanGuard` sync fallback uses `block_on` in a new thread
- **File:** `crates/runtime_core/src/task_manager.rs`, lines 78–88.
- **Label:** HYPOTHESIS
- **Detail:** When no Tokio runtime handle is available during drop, a new thread is spawned with `block_on`. This could deadlock if called from within an async context that holds locks, but this is a hypothesis since the exact call sites were not traced.

### 10. `auth_guard` applies to all guarded routes; default bind address
- **File:** `bins/ide_backend/src/server.rs`, lines 150–165 (`auth_guard`), lines 167–195 (`start_server`).
- **Label:** CONFIRMED
- **Detail:** Bearer token verification via `crate::auth::verify_token_detailed` guards all `/api/*` routes except `/auth/login`. `/health`, `/ws/telemetry`, and static assets are unguarded. `start_server` reads the `NEXUS_BIND` environment variable and defaults to `127.0.0.1:3000` when unset (lines 167–195).

## Recommendations

- **RECOMMENDATION:** Wire `supervise_worker` around all background subsystem spawns in `main.rs` to satisfy the R-5 regression guard and provide backoff/jitter resilience.
- **RECOMMENDATION:** Implement `TempVmoFs::read_file` against the real VMO layer before any auditor agent depends on it.
- **RECOMMENDATION:** Implement the attention score computation in `decoder_layer.rs` (roadmap item 2) before the decoder layer can be used for inference.
- **RECOMMENDATION:** Track the `JoinHandle` from `unified_gateway`'s spawned task and surface panics/errors to the client or at least log them.
- **RECOMMENDATION:** Verify KV-shared layer projection availability at load time to avoid runtime `expect` panics in `forward`.

## Not Traced

MemoryPalace, CorticalLoop, the verifier (`orchestrator_v2::verifier`), and most crates were not traced in this node. Findings above are limited to the five files read.
