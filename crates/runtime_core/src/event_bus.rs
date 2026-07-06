use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    TaskCreated,
    TaskStarted,
    TaskCompleted,
    AgentCrashed,
    ArtifactCommitted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRef {
    pub seq: u64,
    pub kind: EventKind,
    pub object_id: String,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<EventRef>,
}

impl EventBus {
    pub fn new(size: usize) -> Self {
        let (tx, _) = broadcast::channel(size);
        Self { tx }
    }

    pub fn emit(&self, evt: EventRef) {
        let _ = self.tx.send(evt);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventRef> {
        self.tx.subscribe()
    }
}
