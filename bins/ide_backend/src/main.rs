/// Supervises one worker, restarting unexpected exits and panics with bounded backoff.
pub async fn supervise_worker<F, Fut>(
    name: &'static str,
    mut worker_fn: F,
    mut shutdown: broadcast::Receiver<()>,
)
where
    F: FnMut() -> Fut + Send,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    supervise_worker_with_backoff(
        name,
        &mut worker_fn,
        &mut shutdown,
        std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(60),
    )
    .await;
}

async fn supervise_worker_with_backoff<F, Fut>(
    name: &'static str,
    worker_fn: &mut F,
    shutdown: &mut broadcast::Receiver<()>,
    initial_backoff: std::time::Duration,
    max_backoff: std::time::Duration,
)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let mut backoff = initial_backoff;

    loop {
        if shutdown.try_recv().is_ok() {
            tracing::info!(subsystem = name, "Shutdown received before worker start");
            return;
        }

        tracing::info!(subsystem = name, "Starting background subsystem worker");
        let mut worker = tokio::spawn(worker_fn());

        let unexpected_exit = tokio::select! {
            biased;
            _ = shutdown.recv() => {
                tracing::info!(subsystem = name, "Shutdown received; stopping worker");
                worker.abort();
                let _ = worker.await;
                return;
            }
            result = &mut worker => {
                match result {
                    Ok(Ok(())) => {
                        tracing::warn!(subsystem = name, "Worker exited unexpectedly");
                    }
                    Ok(Err(err)) => {
                        tracing::error!(subsystem = name, error = %err, "Worker failed");
                    }
                    Err(err) if err.is_panic() => {
                        tracing::error!(subsystem = name, error = %err, "Worker panicked");
                    }
                    Err(err) => {
                        tracing::error!(subsystem = name, error = %err, "Worker task terminated");
                    }
                }
                true
            }
        };

        if unexpected_exit {
            tokio::select! {
                biased;
                _ = shutdown.recv() => {
                    tracing::info!(subsystem = name, "Shutdown received during restart backoff");
                    return;
                }
                _ = tokio::time::sleep(backoff) => {}
            }
            backoff = std::cmp::min(backoff.saturating_mul(2), max_backoff);
        }
    }
}

fn spawn_supervised_worker<F, Fut>(
    tasks: &mut tokio::task::JoinSet<()>,
    shutdown_tx: &broadcast::Sender<()>,
    name: &'static str,
    worker_fn: F,
)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let shutdown_rx = shutdown_tx.subscribe();
    tasks.spawn(supervise_worker(name, worker_fn, shutdown_rx));
}

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use nexus_runtime_core::cluster::ClusterMesh;
use nexus_runtime_core::distributed_scheduler::DistributedScheduler;
use nexus_runtime_core::mcp::McpServer;
use nexus_runtime_core::memory_cortex::MemoryCortex;
use nexus_runtime_core::orchestrator::Orchestrator;
use nexus_runtime_core::shared_state::VmoState;
use nexus_substrate::memory::MemoryPalace;
use secrecy::SecretString;

mod auth;
mod broker;
mod cluster_api;
mod dream_cycle;
mod edge_worker;
mod fs_watcher;
mod gateway_trait;
mod heartbeat;
mod logger;
mod scheduler;
mod security;
mod self_healing_engine;
mod server;
mod server_terminal;
mod telemetry_bridge;
mod thermal_governor;
mod vitals_handler;
mod ws_bridge;

/// Acquires exclusive process ownership before touching the startup token
/// or the daemon database. Keep the returned File alive for the daemon's
/// entire lifetime; the OS releases flock automatically when it closes.
fn acquire_daemon_lock() -> anyhow::Result<std::fs::File> {
    use std::os::fd::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(".nexus_daemon.lock")?;

    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };

    if result != 0 {
        let error = std::io::Error::last_os_error();
        anyhow::bail!("Another AthenaFusionX daemon owns .nexus_daemon.lock: {}", error);
    }

    tracing::info!("[OWNERSHIP] Exclusive daemon lock acquired.");
    Ok(file)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    logger::init();

    tracing::info!("==================================================");
    tracing::info!(">> NEXUS SINGULARITY v4.0 — DAEMON ONLINE");
    tracing::info!("==================================================");

    // Must precede token creation and database initialization.
    // Keep this handle alive until main exits.
    let _daemon_lock = acquire_daemon_lock()?;
    let _auth_key = auth::load_key_at_startup()?; // fail fast if NEXUS_AUTH_KEY missing/short

    security::write_startup_token()?;
    let api_key_str = std::env::var("GEMINI_API_KEY").unwrap_or_default().trim().to_string();
    let api_key = SecretString::from(api_key_str.clone());
    let req_client = reqwest::Client::new();

    let (tx, _) = broadcast::channel::<String>(256);

    let palace = Arc::new(MemoryPalace::new("nexus_daemon.db").await?);
    match palace.recover_or_abort_orphaned_plans().await {
        Ok(r) => {
            if !r.recoverable.is_empty() {
                tracing::warn!(
                    "[BOOT] {} recoverable plan(s) from dead owner (not auto-resumed):",
                    r.recoverable.len()
                );
                for rec in &r.recoverable {
                    let excerpt: String = rec.root_directive.chars().take(80).collect();
                    tracing::warn!(
                        "[BOOT]   id={} graph_bytes={} directive={:?}",
                        rec.id,
                        rec.graph_bytes,
                        excerpt
                    );
                }
            }
            if !r.aborted.is_empty() {
                tracing::warn!("[BOOT] Aborted {} orphaned plan(s).", r.aborted.len());
            }
            if !r.skipped_live.is_empty() {
                tracing::info!(
                    "[BOOT] Preserved {} plan(s) owned by live process.",
                    r.skipped_live.len()
                );
            }
            if !r.stale_heartbeat.is_empty() {
                tracing::warn!(
                    "[BOOT] {} live-owner plan(s) have stale heartbeats; preserved, not aborted.",
                    r.stale_heartbeat.len()
                );
                for id in &r.stale_heartbeat {
                    tracing::warn!("[BOOT]   stale heartbeat: plan_id={}", id);
                }
            }
        }
        Err(e) => tracing::error!("[BOOT] Recovery failed: {}", e),
    }

    // 👇 The crucial fix: We clone the palace Arc so it can be shared!
    let memory = Arc::new(MemoryCortex::new(palace.clone()));
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));
    let cluster = Arc::new(ClusterMesh::new());
    let dist = Arc::new(DistributedScheduler::new(cluster.clone()));
    let orchestrator = Arc::new(Orchestrator::new());

    let monitor = Arc::new(Mutex::new(nexus_ide_backend::system_monitor::SystemMonitor::new()));
    let resonance = Arc::new(std::sync::atomic::AtomicU32::new(1000));

    // Supervised workers share one shutdown channel. Each worker receives
    // its own subscription so it can stop independently and be restarted.
    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(16);
    let mut worker_tasks = tokio::task::JoinSet::new();

    let worker_shutdown = shutdown_tx.clone();
    let worker_palace = palace.clone();
    let worker_tx = tx.clone();
    let worker_client = req_client.clone();
    let worker_api_key = api_key_str.clone();
    let worker_orchestrator = orchestrator.clone();
    spawn_supervised_worker(
        &mut worker_tasks,
        &shutdown_tx,
        "dream_cycle",
        move || dream_cycle::run(
            worker_palace.clone(),
            worker_tx.clone(),
            worker_client.clone(),
            worker_api_key.clone(),
            worker_orchestrator.clone(),
            worker_shutdown.subscribe(),
        ),
    );

    let worker_tx = tx.clone();
    let worker_monitor = monitor.clone();
    let self_healing_shutdown = shutdown_tx.clone();
    spawn_supervised_worker(
        &mut worker_tasks,
        &shutdown_tx,
        "self_healing_engine",
        move || self_healing_engine::run(
            worker_tx.clone(),
            worker_monitor.clone(),
            self_healing_shutdown.subscribe(),
        ),
    );

    let worker_tx = tx.clone();
    let worker_monitor = monitor.clone();
    let worker_resonance = resonance.clone();
    let telemetry_shutdown = shutdown_tx.clone();
    spawn_supervised_worker(
        &mut worker_tasks,
        &shutdown_tx,
        "telemetry_bridge",
        move || telemetry_bridge::run(
            worker_tx.clone(),
            worker_monitor.clone(),
            worker_resonance.clone(),
            telemetry_shutdown.subscribe(),
        ),
    );

    let thermal_shutdown = shutdown_tx.clone();
    spawn_supervised_worker(
        &mut worker_tasks,
        &shutdown_tx,
        "thermal_governor",
        move || thermal_governor::run(thermal_shutdown.subscribe()),
    );

    let heartbeat_tx = tx.clone();
    let heartbeat_shutdown = shutdown_tx.clone();
    spawn_supervised_worker(
        &mut worker_tasks,
        &shutdown_tx,
        "heartbeat",
        move || heartbeat::run(
            heartbeat_tx.clone(),
            heartbeat_shutdown.subscribe(),
        ),
    );

    let state = Arc::new(server::AppState {
        client: req_client,
        monitor,
        orchestrator,
        memory,
        api_key,
        tx,
        workspace_root: PathBuf::from("."),
        vmo_state,
        cluster,
        dist,
        mcp: Arc::new(McpServer::new()),
    });

    // 🌟 Boot WASM Sidecars 🌟
    let sidecars_dir = PathBuf::from("sidecars");
    let mut sidecar_tasks = tokio::task::JoinSet::new();
    if let Ok(entries) = std::fs::read_dir(&sidecars_dir) {
        if let Ok(sidecar_host) = nexus_runtime_core::sidecar_host::SidecarHost::new(true) {
            let sidecar_host = Arc::new(sidecar_host);
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    tracing::info!(">> [SIDECAR] Loading external cognitive module: {:?}", path);
                    let host = sidecar_host.clone();
                    sidecar_tasks.spawn_blocking(move || {
                        if let Err(e) = host.load_and_run(&path) {
                            let err_str = e.to_string();
                            if err_str.contains("_start")
                                || err_str.contains("failed to find function export")
                            {
                                tracing::debug!(
                                    "[SIDECAR] Loaded library module {:?} (no _start export)",
                                    path
                                );
                            } else {
                                tracing::error!("[SIDECAR] Failed to execute {:?}: {}", path, e);
                            }
                        }
                    });
                }
            }
        }
    }

    tokio::spawn(async move {
        while let Some(res) = sidecar_tasks.join_next().await {
            if let Err(e) = res {
                if e.is_panic() {
                    tracing::error!("\x1b[31m[SIDECAR PANIC] A cognitive module worker thread crashed: {}\x1b[0m", e);
                }
            }
        }
    });


    {
        let palace = palace.clone();
        let shutdown_tx = shutdown_tx.clone();
        let sig_handle = tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            let (Ok(mut term), Ok(mut int)) =
                (signal(SignalKind::terminate()), signal(SignalKind::interrupt()))
            else {
                return;
            };
            tokio::select! {
                _ = term.recv() => tracing::info!("[SHUTDOWN] Received SIGTERM"),
                _ = int.recv() => tracing::info!("[SHUTDOWN] Received SIGINT"),
            }
            match palace.abort_plans_owned_by(std::process::id() as i64).await {
                Ok(n) => tracing::warn!("[SHUTDOWN] Aborted {} in-flight plan(s).", n),
                Err(e) => tracing::error!("[SHUTDOWN] Abort failed: {}", e),
            }
            let _ = shutdown_tx.send(());
            tracing::info!("[SHUTDOWN] Graceful server shutdown requested.");
        });

        tokio::spawn(async move {
            if let Err(e) = sig_handle.await {
                if e.is_panic() {
                    tracing::error!("[SHUTDOWN] FATAL: Signal handler task panicked: {}", e);
                }
            }
        });
    }

    tracing::info!(">> Core Subsystems online. Binding Axum API Gateway to 0.0.0.0:3000...");
    server::start_server(state, shutdown_rx).await;

    // Also signal workers if the server returns because of an internal error.
    let _ = shutdown_tx.send(());
    while let Some(result) = worker_tasks.join_next().await {
        if let Err(err) = result {
            tracing::error!("[SHUTDOWN] Worker supervisor task failed: {}", err);
        }
    }
    tracing::info!("[SHUTDOWN] All supervised workers have terminated.");

    Ok(())
}

#[cfg(test)]
mod regression_guards {
    use super::supervise_worker_with_backoff;
    use std::future::pending;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;
    use tokio::sync::{broadcast, oneshot};

    #[tokio::test]
    async fn supervisor_restarts_after_unexpected_clean_exit_and_stops_on_shutdown() {
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(4);
        let (started_tx, started_rx) = oneshot::channel();
        let started_tx = Arc::new(Mutex::new(Some(started_tx)));
        let calls = Arc::new(AtomicUsize::new(0));
        let worker_calls = calls.clone();

        let mut worker_fn = {
            let started_tx = started_tx.clone();
            move || {
                let call = worker_calls.fetch_add(1, Ordering::SeqCst) + 1;
                let started_tx = started_tx.clone();

                async move {
                    if call == 1 {
                        return Ok(());
                    }

                    if let Some(tx) = started_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }

                    pending::<anyhow::Result<()>>().await
                }
            }
        };

        let supervisor = tokio::spawn(async move {
            supervise_worker_with_backoff(
                "test-clean-exit",
                &mut worker_fn,
                &mut shutdown_rx,
                Duration::from_millis(5),
                Duration::from_millis(20),
            )
            .await;
        });

        started_rx.await.expect("second worker start was not observed");
        let _ = shutdown_tx.send(());
        tokio::time::timeout(Duration::from_secs(1), supervisor)
            .await
            .expect("supervisor did not stop after shutdown")
            .expect("supervisor task panicked");

        assert!(calls.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn supervisor_restarts_after_panic_and_stops_on_shutdown() {
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(4);
        let (started_tx, started_rx) = oneshot::channel();
        let started_tx = Arc::new(Mutex::new(Some(started_tx)));
        let calls = Arc::new(AtomicUsize::new(0));
        let worker_calls = calls.clone();

        let mut worker_fn = {
            let started_tx = started_tx.clone();
            move || {
                let call = worker_calls.fetch_add(1, Ordering::SeqCst) + 1;
                let started_tx = started_tx.clone();

                async move {
                    if call == 1 {
                        panic!("intentional supervisor regression-test panic");
                    }

                    if let Some(tx) = started_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }

                    pending::<anyhow::Result<()>>().await
                }
            }
        };

        let supervisor = tokio::spawn(async move {
            supervise_worker_with_backoff(
                "test-panic",
                &mut worker_fn,
                &mut shutdown_rx,
                Duration::from_millis(5),
                Duration::from_millis(20),
            )
            .await;
        });

        started_rx.await.expect("worker was not restarted after panic");
        let _ = shutdown_tx.send(());
        tokio::time::timeout(Duration::from_secs(1), supervisor)
            .await
            .expect("supervisor did not stop after shutdown")
            .expect("supervisor task panicked");

        assert!(calls.load(Ordering::SeqCst) >= 2);
    }
}
