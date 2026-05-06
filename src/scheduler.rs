use crate::distributed_scheduler::DistributedScheduler;
use nexus_substrate::memory::MemoryPalace;
use crate::orchestrator::Orchestrator;
use crate::server::VmoState;
use crate::system_monitor::SystemMonitor;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct NexusTask {
    pub payload: String,
    pub priority: u8,
}

pub struct MasterSupervisor;

impl MasterSupervisor {
    pub fn ignite(
        tx: broadcast::Sender<String>, client: reqwest::Client, api_key: String, orchestrator: Arc<Orchestrator>,
        _resonance_meter: Arc<AtomicU32>, mut monitor: SystemMonitor, memory: Arc<Mutex<MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>, dist: Arc<DistributedScheduler>,
    ) -> mpsc::Sender<NexusTask> {
        let (task_tx, mut task_rx) = mpsc::channel::<NexusTask>(32);
        let task_tx_requeue = task_tx.clone();

        tokio::spawn(async move {
            while let Some(task) = task_rx.recv().await {
                let vitals = monitor.get_vitals();

                if vitals.cpu_load > 0.85 || vitals.temperature > 68.0 {
                    warn!("[SCHEDULER] Substrate hot. Re-queuing task.");
                    // ⚡ FIX: Re-queue task asynchronously instead of dropping it
                    let requeue_tx = task_tx_requeue.clone();
                    let t_clone = task.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
                        let _ = requeue_tx.send(t_clone).await;
                    });
                    continue;
                }

                let cl = client.clone();
                let ak = api_key.clone();
                let orch = orchestrator.clone();
                let mem = memory.clone();
                let vs = vmo_state.clone();
                let task_tx_clone = tx.clone();
                let d = dist.clone();

                tokio::spawn(async move {
                    let _ = crate::task_manager::send_task(orch, &cl, &ak, &task.payload, mem, vs, task_tx_clone, d).await;
                });
            }
        });
        task_tx
    }
}
