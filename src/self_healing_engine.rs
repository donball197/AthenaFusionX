use crate::{
    inquisitor::Inquisitor,
    memory::MemoryPalace,
    server::AppState,
    neural_agent::ContextPacket,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Semaphore, Mutex};
use tokio::time::{sleep, Duration};

static COMPILE_LOCK: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

pub struct SelfHealingEngine {
    state: Arc<AppState>,
}

impl SelfHealingEngine {
    pub fn new(state: Arc<AppState>, _memory: Arc<Mutex<MemoryPalace>>) -> Self {
        Self { state }
    }

    pub async fn ignite_autonomic_loop(self) {
        println!("\x1b[36m>> [SELF-HEALING v3.2] Neural Suture System Active\x1b[0m");

        loop {
            sleep(Duration::from_secs(300)).await;
            let vitals = self.state.monitor.lock().await.get_vitals();

            // Thermal safety gate
            if vitals.temperature > 70.0 || vitals.cpu_load > 0.8 { continue; }

            if let Ok(_guard) = COMPILE_LOCK.clone().try_acquire_owned() {
                if let Ok(log) = Inquisitor::audit_backend() {
                    if log != "NOMINAL" {
                        println!("\x1b[31m[HEALING] Fracture detected. Invoking Gemma 4 Suture...\x1b[0m");
                        self.attempt_neural_suture(&log).await;
                    }
                }
            }
        }
    }

    async fn attempt_neural_suture(&self, error_log: &str) {
        let prompt = format!(
            "CONTEXT: Rust/ARM64 Build Failure\nERROR_LOG: {}\nTASK: Identify the file and provide a fix in <SUTURE> tags.",
            error_log
        );
        
        let client = &self.state.client;
        let api_key = &self.state.api_key;
        let ctx = ContextPacket::default();

        if let Ok((response, _)) = self.state.agent.prepare_and_run(client, api_key, &prompt, ctx, 0.2, Some("gemma-4-31b-it")).await {
            if response.contains("<SUTURE>") {
                println!("\x1b[32m[HEALING] Suture payload received. Substrate repair logic primed.\x1b[0m");
            }
        }
    }
}
