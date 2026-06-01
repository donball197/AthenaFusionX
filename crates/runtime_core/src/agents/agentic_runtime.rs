use std::sync::Arc;
use tokio::sync::Mutex;
use crate::shared_state::VmoState;
use crate::distributed_scheduler::DistributedScheduler;
use nexus_substrate::memory::MemoryPalace;

pub struct SubstrateContext {
    pub memory: Arc<Mutex<MemoryPalace>>,
    pub vmo_state: Arc<Mutex<VmoState>>,
    pub dist: Arc<DistributedScheduler>,
}

use tokio::sync::{mpsc, oneshot};

pub type RuntimeResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub enum AgentCommand {
    DispatchTask {
        prompt: String,
        respond_to: oneshot::Sender<RuntimeResult<()>>,
    },
}

#[derive(Clone)]
pub struct AgenticRuntime {
    cmd_sender: mpsc::Sender<AgentCommand>,
}

impl AgenticRuntime {
    pub fn new(execution_budget_ms: u64) -> Self {
        let (cmd_sender, mut cmd_receiver) = mpsc::channel::<AgentCommand>(32);
        tokio::spawn(async move {
            while let Some(cmd) = cmd_receiver.recv().await {
                match cmd {
                    AgentCommand::DispatchTask { prompt, respond_to } => {
                        let result = Self::process_execution_loop(&prompt, execution_budget_ms).await;
                        let _ = respond_to.send(result);
                    }
                }
            }
        });
        Self { cmd_sender }
    }

    pub async fn dispatch(&self, prompt: &str) -> RuntimeResult<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .send(AgentCommand::DispatchTask {
                prompt: prompt.to_string(),
                respond_to: tx,
            })
            .await
            .map_err(|e| format!("Failed to queue task: {}", e))?;
        rx.await.map_err(|e| format!("Runtime channel dropped: {}", e))?
    }

    async fn process_execution_loop(prompt: &str, _budget: u64) -> RuntimeResult<()> {
        let raw_source = "<html><body><div id='editor' contenteditable='true'></div></body></html>";
        let _compressed_context = DomBridge::minify_dom(raw_source)?;
        tracing::info!("[COGNITIVE FSM] Step evaluation processed: {}", prompt);
        Ok(())
    }
}


// ================================================
// COMPATIBILITY STUBS (kept minimal)
// ================================================

#[derive(Debug, Clone, PartialEq)]
pub enum InferenceStrategy {
    DeepReasoning, ShallowReasoning, ReactiveFlash, EmergencyEvac,
}

#[derive(Debug)]
pub struct DuetResourceGuard;
impl DuetResourceGuard { pub fn new() -> Self { Self } }
impl Default for DuetResourceGuard { fn default() -> Self { Self::new() } }

#[derive(Debug)]
pub struct SystemVitals {
    pub cpu: f32,
    pub temp: f32,
}

pub struct DomBridge;
impl DomBridge {
    pub fn minify_dom(_raw: &str) -> anyhow::Result<String> {
        Ok("minified".to_string())
    }
}
