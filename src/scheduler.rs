use crate::orchestrator::Orchestrator;
use crate::system_monitor::SystemMonitor;
use crate::memory::MemoryPalace;
use tokio::sync::{broadcast, mpsc, Mutex};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use serde_json::json;

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
        resonance_meter: Arc<AtomicU32>,
        monitor: SystemMonitor,
        memory: Arc<Mutex<MemoryPalace>>
    ) -> mpsc::Sender<NexusTask> {
        let (task_tx, mut task_rx) = mpsc::channel::<NexusTask>(32);
        
        tokio::spawn(async move {
            let _ = tx.send(json!({
                "type": "LOG_EVENT",
                "payload": { "level": "INFO", "message": "Master Supervisor ignited." }
            }).to_string());

            while let Some(task) = task_rx.recv().await {
                let _current_resonance = resonance_meter.load(Ordering::Relaxed);
                let vitals = monitor.get_vitals();

                if vitals.cpu_load > 90.0 || vitals.temperature > 75.0 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
                    continue;
                }

                let cl = client.clone();
                let ak = api_key.clone();
                let orch = orchestrator.clone();
                let mem = memory.clone();
                let task_tx = tx.clone(); 

                tokio::spawn(async move {
                    let _ = crate::task_manager::send_task(
                        orch.agent.clone(), &cl, &ak, &task.payload, mem, task_tx
                    ).await;
                });
            }
        });

        task_tx
    }
}
