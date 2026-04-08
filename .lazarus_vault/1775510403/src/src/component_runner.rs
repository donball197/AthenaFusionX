use crate::nexus_error::NexusError;
use crate::agent_protocol::AgentAction;
use crate::executor::Executor;
use bitflags::bitflags;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capabilities: u32 {
        const READ_FS      = 0b0000_0001;
        const WRITE_FS     = 0b0000_0010;
        const NETWORK      = 0b0000_0100;
        const SPAWN_DRONE  = 0b0000_1000;
        const SYSTEM_EXEC  = 0b0001_0000;
        const MEMORY_OPS   = 0b0010_0000;
        const VISION       = 0b0100_0000;
        const ZIG_FFI      = 0b1000_0000;
        const ALL          = 0b1111_1111;
    }
}

pub enum ComponentMessage {
    ExecuteActions {
        actions: Vec<AgentAction>,
        reply: oneshot::Sender<Result<Vec<String>, NexusError>>,
    },
    ZigFfiCall {
        op: Box<dyn FnOnce() -> Result<String, String> + Send + 'static>,
        reply: oneshot::Sender<Result<String, NexusError>>,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct AgentHandle {
    pub tx: mpsc::Sender<ComponentMessage>,
    pub id: String,
    pub capabilities: Capabilities,
}

impl AgentHandle {
    pub async fn execute_actions(&self, actions: Vec<AgentAction>) -> Result<Vec<String>, NexusError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(ComponentMessage::ExecuteActions { actions, reply: reply_tx }).await;
        reply_rx.await.map_err(|_| NexusError::Other("Actor reply closed".into()))?
    }

    pub async fn zig_ffi_call<F>(&self, op: F) -> Result<String, NexusError>
    where
        F: FnOnce() -> Result<String, String> + Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(ComponentMessage::ZigFfiCall { op: Box::new(op), reply: reply_tx }).await;
        reply_rx.await.map_err(|_| NexusError::Other("Actor reply closed".into()))?
    }
}

pub enum SupervisionEvent {
    ChildFailed { id: String, error: String },
    ChildShutdown { id: String },
}

pub struct ActorSupervisor {
    _children: Vec<AgentHandle>,
    _lifecycle_tx: broadcast::Sender<String>,
}

impl ActorSupervisor {
    pub fn new(lifecycle_tx: broadcast::Sender<String>) -> (Self, mpsc::Receiver<SupervisionEvent>) {
        let (_sup_tx, sup_rx) = mpsc::channel(32);
        let supervisor = Self {
            _children: Vec::new(),
            _lifecycle_tx: lifecycle_tx,
        };
        (supervisor, sup_rx)
    }

    pub fn spawn_child(
        &mut self,
        id: &str,
        caps: Capabilities,
        agent: Arc<crate::neural_agent::MicroAgent>,
        lifecycle_tx: broadcast::Sender<String>,
    ) -> AgentHandle {
        let handle = AgentComponent::spawn(id, caps, agent, lifecycle_tx);
        self._children.push(handle.clone());
        handle
    }

    pub async fn monitor(&self, mut rx: mpsc::Receiver<SupervisionEvent>) {
        while let Some(event) = rx.recv().await {
            match event {
                SupervisionEvent::ChildFailed { id, error } => {
                    println!(">> [SUPERVISOR] Child {} failed: {}. Restarting...", id, error);
                }
                SupervisionEvent::ChildShutdown { id } => {
                    println!(">> [SUPERVISOR] Child {} shutdown cleanly.", id);
                }
            }
        }
    }
}

pub struct AgentComponent {
    pub id: String,
    pub capabilities: Capabilities,
    pub agent: Arc<crate::neural_agent::MicroAgent>,
    pub lifecycle_tx: broadcast::Sender<String>,
    pub mailbox: mpsc::Receiver<ComponentMessage>,
    pub self_tx: mpsc::Sender<ComponentMessage>,
}

impl AgentComponent {
    pub fn spawn(
        id: &str,
        caps: Capabilities,
        agent: Arc<crate::neural_agent::MicroAgent>,
        lifecycle_tx: broadcast::Sender<String>,
    ) -> AgentHandle {
        let (tx, rx) = mpsc::channel(32);
        let actor = Self {
            id: id.to_string(),
            capabilities: caps,
            agent,
            lifecycle_tx,
            mailbox: rx,
            self_tx: tx.clone(),
        };
        tokio::spawn(actor.run());
        AgentHandle { tx, id: id.to_string(), capabilities: caps }
    }

    async fn run(mut self) {
        while let Some(msg) = self.mailbox.recv().await {
            match msg {
                ComponentMessage::ExecuteActions { actions, reply } => {
                    let result = Executor::execute_handle(&self.to_handle(), actions).await;
                    let _ = reply.send(result);
                }
                ComponentMessage::ZigFfiCall { op, reply } => {
                    let result = if self.capabilities.contains(Capabilities::ZIG_FFI) {
                        op().map_err(|e| NexusError::Other(format!("ZIG_FFI: {}", e)))
                    } else {
                        Err(NexusError::SecurityViolation { message: format!("{} lacks ZIG_FFI", self.id), cvss: 7.5 })
                    };
                    let _ = reply.send(result);
                }
                ComponentMessage::Shutdown => break,
            }
        }
    }

    fn to_handle(&self) -> AgentHandle {
        AgentHandle {
            tx: self.self_tx.clone(),
            id: self.id.clone(),
            capabilities: self.capabilities,
        }
    }
}
