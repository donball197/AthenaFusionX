use crate::memory::MemoryPalace;
use crate::orchestrator::Orchestrator;
use crate::system_monitor::SystemMonitor;
use crate::server::VmoState;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct NexusTask {
    pub payload: String,
    pub priority: u8,
}

pub struct MasterSupervisor;

impl MasterSupervisor {
    pub fn ignite(
        tx: broadcast::Sender<String>,
        client: reqwest::Client,
        api_key: String,
        orchestrator: Arc<Orchestrator>,
        _resonance_meter: Arc<AtomicU32>, // ⚡ PREFIXED: Intentional unused variable
        mut monitor: SystemMonitor,
        memory: Arc<Mutex<MemoryPalace>>,
        vmo_state: Arc<Mutex<VmoState>>,
    ) -> mpsc::Sender<NexusTask> {
        let (task_tx, mut task_rx) = mpsc::channel::<NexusTask>(32);

        tokio::spawn(async move {
            while let Some(task) = task_rx.recv().await {
                let vitals = monitor.get_vitals();

                // Throttling logic remains active to protect the metal
                if vitals.cpu_load > 0.85 || vitals.temperature > 68.0 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    continue;
                }

                let cl = client.clone();
                let ak = api_key.clone();
                let orch = orchestrator.clone();
                let mem = memory.clone();
                let vs = vmo_state.clone();
                let task_tx_clone = tx.clone();

                tokio::spawn(async move {
                    let _ = crate::task_manager::send_task(
                        orch.clone(),
                        &cl,
                        &ak,
                        &task.payload,
                        mem,
                        vs,
                        task_tx_clone,
                    ).await;
                });
            }
        });
        task_tx
    }
}
