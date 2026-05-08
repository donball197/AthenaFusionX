use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventKind {
    TaskCreated,
    TaskStarted,
    TaskCompleted,
    AgentCrashed,
    MemoryCommitted,
    ThermalThrottle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRef {
    pub sequence: u64,
    pub kind: EventKind,
    pub object_id: String,
}

pub type EventBusTx = tokio::sync::broadcast::Sender<EventRef>;
pub type EventBusRx = tokio::sync::broadcast::Receiver<EventRef>;
