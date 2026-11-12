# Worker Shape Audit

Scope: `bins/ide_backend/src/{dream_cycle,self_healing_engine,telemetry_bridge,thermal_governor,heartbeat}.rs`. MemoryPalace, CorticalLoop, the verifier and most crates were not traced. Findings labeled CONFIRMED (read directly), LIKELY, HYPOTHESIS, RECOMMENDATION.

## 1. dream_cycle.rs

1. spawn signature: CONFIRMED — there is no `spawn()` function in this file. The public entry point is `run(palace: Arc<MemoryPalace>, tx: broadcast::Sender<String>, client: reqwest::Client, api_key: String, orchestrator: Arc<nexus_runtime_core::orchestrator::Orchestrator>, mut shutdown: broadcast::Receiver<()>) -> anyhow::Result<()>` (lines 6-13).
2. Body shape: CONFIRMED — internal infinite loop (`loop { ... }`, line 17). A 30-minute ticker is created at line 14 (`let mut ticker = interval(Duration::from_secs(30 * 60));`, line 14) and first tick awaited at line 15. Each iteration waits on the ticker or shutdown, then runs a synthesis pass (the `cycle` async block, lines 26-73); shutdown returns `Ok(())`.
3. One-attempt `run()`: RECOMMENDATION — extract the body of the `cycle` async block (lines 26-73) into a single `run_once(...)` returning `anyhow::Result<()>`, dropping the outer `loop` (line 17) and ticker (line 14); the shutdown select becomes unnecessary for a one-shot.
4. supervise_worker wiring: REQUIRES REFACTOR — no `spawn()` exists to wrap; caller must be updated to call `run()` directly or a thin `spawn()` shim added. The function already returns `anyhow::Result<()>`, so the result type is compatible.

## 2. self_healing_engine.rs

1. spawn signature: CONFIRMED — no `spawn()`; public `run(tx: broadcast::Sender<String>, monitor: Arc<Mutex<SystemMonitor>>, mut shutdown: broadcast::Receiver<()>) -> anyhow::Result<()>` (lines 5-9).
2. Body shape: CONFIRMED — internal infinite loop (`loop`, line 12), 60s ticker (`let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(60));`, line 10).
3. One-attempt `run()`: RECOMMENDATION — remove the `loop` (line 12) and ticker (line 10); perform one vitals check + throttle decision, return `Ok(())`.
4. supervise_worker wiring: REQUIRES REFACTOR — no `spawn()` shim; signature compatible with `anyhow::Result<()>`.

## 3. telemetry_bridge.rs

1. spawn signature: CONFIRMED — no `spawn()`; public `run(tx: broadcast::Sender<String>, monitor: Arc<Mutex<nexus_ide_backend::system_monitor::SystemMonitor>>, resonance: Arc<std::sync::atomic::AtomicU32>, mut shutdown: broadcast::Receiver<()>) -> anyhow::Result<()>` (lines 7-12).
2. Body shape: CONFIRMED — internal infinite loop (`loop`, line 15), 3s ticker (`let mut ticker = interval(Duration::from_secs(3));`, line 13).
3. One-attempt `run()`: RECOMMENDATION — single vitals read + broadcast, return `Ok(())`.
4. supervise_worker wiring: REQUIRES REFACTOR — no `spawn()` shim; signature compatible.

## 4. thermal_governor.rs

1. spawn signature: CONFIRMED — no `spawn()`; public `run(mut shutdown: broadcast::Receiver<()>) -> anyhow::Result<()>` (line 9).
2. Body shape: CONFIRMED — internal infinite loop (`loop`, line 13), 10s ticker (`let mut ticker = interval(Duration::from_secs(10));`, line 10).
3. One-attempt `run()`: RECOMMENDATION — single refresh + store atomics, return `Ok(())`.
4. supervise_worker wiring: REQUIRES REFACTOR — no `spawn()` shim; signature compatible.

## 5. heartbeat.rs

1. spawn signature: CONFIRMED — no `spawn()`; public `run(tx: broadcast::Sender<String>, mut shutdown: broadcast::Receiver<()>) -> anyhow::Result<()>` (lines 7-10).
2. Body shape: CONFIRMED — internal infinite loop (`loop`, line 13), 5s ticker (`let mut ticker = interval(Duration::from_secs(5));`, line 11).
3. One-attempt `run()`: RECOMMENDATION — single ping send, return `Ok(())`.
4. supervise_worker wiring: REQUIRES REFACTOR — no `spawn()` shim; signature compatible.

## Cross-cutting

CONFIRMED: None of the five modules expose a `spawn()` function; all expose `run()` already returning `anyhow::Result<()>` with an internal infinite loop and shutdown handling. Wiring `supervise_worker` is therefore not a drop-in: each needs either a `spawn()` shim or caller changes, plus conversion of the loop body into a one-attempt `run()` (or a separate `run_once()`). HYPOTHESIS: `supervise_worker` expects a restartable one-shot; the current `run()` functions never return except on shutdown, so they cannot be supervised as-is.
