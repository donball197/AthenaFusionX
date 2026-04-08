use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, broadcast, Mutex};
use tokio::time::{sleep, Duration};
use crate::neural_agent::ContextPacket;
use crate::orchestrator::Orchestrator;
use crate::system_monitor::SystemMonitor;
use crate::mission_queue::MissionQueue;
use crate::thermal_governor::ThermalGovernor;
use crate::memory::MemoryPalace;
use std::sync::atomic::AtomicU32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Domain { UI, Code, Build, Security }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority { Low, Normal, High, Critical }

#[derive(Debug)]
pub struct NexusTask {
    pub priority: TaskPriority, // SUTURE: Moved to first position for correct sorting
    pub id: String,
    pub domain: Domain,
    pub payload: String,
    pub context: ContextPacket,
    pub response_tx: Option<oneshot::Sender<String>>,
}

// Manual Ord implementation to ensure Priority takes precedence over ID
impl Ord for NexusTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority).then_with(|| self.id.cmp(&other.id))
    }
}

impl PartialOrd for NexusTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

impl PartialEq for NexusTask {
    fn eq(&self, other: &Self) -> bool { self.priority == other.priority && self.id == other.id }
}

impl Eq for NexusTask {}

pub struct MasterSupervisor;

impl MasterSupervisor {
    pub fn ignite(
        b_tx: broadcast::Sender<String>,
        client: reqwest::Client,
        api_key: String,
        orchestrator: Arc<Orchestrator>,
        _resonance: Arc<AtomicU32>,
        _monitor: SystemMonitor,
        memory: Arc<Mutex<MemoryPalace>>,
    ) -> mpsc::Sender<NexusTask> {
        let (global_tx, mut global_rx) = mpsc::channel::<NexusTask>(128);

        tokio::spawn(async move {
            let queue = match MissionQueue::new(".nexus_missions.db").await {
                Ok(q) => q,
                Err(e) => {
                    let _ = b_tx.send(format!("!! [QUEUE FAULT] {}", e));
                    return;
                }
            };

            let governor = ThermalGovernor::new();
            let mut heap: std::collections::BinaryHeap<NexusTask> = std::collections::BinaryHeap::new();

            loop {
                while let Ok(task) = global_rx.try_recv() {
                    heap.push(task);
                }

                governor.apply_cooldown().await;

                if let Some(task) = heap.pop() {
                    let orch = orchestrator.clone();
                    let cl = client.clone();
                    let ak = api_key.clone();
                    let q = queue.clone();
                    let mem = memory.clone();
                    let task_id = task.id.clone();
                    let b_tx_clone = b_tx.clone();

                    tokio::spawn(async move {
                        let _ = q.update_status(&task_id, "running").await;
                        let _ = b_tx_clone.send(format!("\x1b[35m>> [DRONE ACTIVE] Executing Task: {}\x1b[0m", task_id));

                        let result = crate::task_manager::send_task(
                            orch.agent.clone(), &cl, &ak, &task.payload, mem
                        ).await;
                        
                        let _ = q.update_status(&task_id, "done").await;
                        let _ = b_tx_clone.send(format!("\x1b[35m>> [DRONE COMPLETE] Task: {}\x1b[0m", task_id));

                        if let Some(tx) = task.response_tx { let _ = tx.send(result); }
                    });
                }
                sleep(Duration::from_millis(250)).await;
            }
        });

        global_tx
    }
}
