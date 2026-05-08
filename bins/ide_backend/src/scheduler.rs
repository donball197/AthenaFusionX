use crate::system_monitor::SystemMonitor;
use nexus_runtime_core::{
    distributed_scheduler::DistributedScheduler, orchestrator::Orchestrator, shared_state::VmoState,
};
use nexus_substrate::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct NexusTask;

pub struct MasterSupervisor;

impl MasterSupervisor {
    #[allow(clippy::too_many_arguments)]
pub fn ignite(
        _tx: tokio::sync::broadcast::Sender<String>,
        _client: reqwest::Client,
        _api_key: String,
        _orch: Arc<Orchestrator>,
        _resonance: Arc<std::sync::atomic::AtomicU32>,
        _monitor: SystemMonitor,
        _memory: Arc<tokio::sync::Mutex<MemoryPalace>>,
        _vmo_state: Arc<tokio::sync::Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> mpsc::Sender<NexusTask> {
        let (tx, _rx) = mpsc::channel(100);
        println!("\x1b[35m>> [SCHEDULER] Master Supervisor Ignited.\x1b[0m");
        tx
    }
}
