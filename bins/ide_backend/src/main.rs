
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

    let result = unsafe {
        libc::flock(
            file.as_raw_fd(),
            libc::LOCK_EX | libc::LOCK_NB,
        )
    };

    if result != 0 {
        let error = std::io::Error::last_os_error();
        anyhow::bail!(
            "Another AthenaFusionX daemon owns .nexus_daemon.lock: {}",
            error
        );
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
                        rec.id, rec.graph_bytes, excerpt
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


    // 🌟 Boot all Subconscious Background Threads 🌟
    dream_cycle::spawn(
        palace.clone(),
        tx.clone(),
        req_client.clone(),
        api_key_str.clone(),
        orchestrator.clone(),
    );
    self_healing_engine::spawn(
        orchestrator.clone(),
        req_client.clone(),
        api_key_str.clone(),
        memory.clone(),
        vmo_state.clone(),
        tx.clone(),
        dist.clone(),
        monitor.clone(),
    );
    telemetry_bridge::spawn(tx.clone(), monitor.clone(), resonance.clone());
    thermal_governor::spawn();
    heartbeat::spawn(tx.clone());

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
        let sig_handle = tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            let (Ok(mut term), Ok(mut int)) =
                (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) else { return };
            tokio::select! { _ = term.recv() => {}, _ = int.recv() => {} }
            match palace.abort_plans_owned_by(std::process::id() as i64).await {
                Ok(n) => tracing::warn!("[SHUTDOWN] Aborted {} in-flight plan(s).", n),
                Err(e) => tracing::error!("[SHUTDOWN] Abort failed: {}", e),
            }
            std::process::exit(0);
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
    server::start_server(state).await;

    Ok(())
}
