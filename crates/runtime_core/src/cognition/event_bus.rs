//! Cognitive Event Bus – Phase 8B v2
//! Async broadcast + pluggable EventStore.

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMeta {
    pub event_id: Uuid,
    pub timestamp_ms: u64,
    pub source: String,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
}

impl EventMeta {
    pub fn new(source: &str) -> Self {
        let ts =
            SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        Self {
            event_id: Uuid::new_v4(),
            timestamp_ms: ts,
            source: source.to_string(),
            correlation_id: None,
            causation_id: None,
        }
    }

    pub fn with_correlation(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CognitiveEventKind {
    Observation { content: String },
    BeliefUpdated { hypothesis_id: Uuid, new_score: f32 },
    GoalActivated { goal_id: Uuid },
    GoalCompleted { goal_id: Uuid },
    ReflectionCompleted { lessons: Vec<String> },
    MissionFinished { success: bool },
    StrategyChanged { strategy: String },
    Interrupt { reason: String },
    AttentionShift { focus: String },
    ResourceBudgetLow { remaining: f32 },
    DecisionMade { decision_summary: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveEvent {
    pub meta: EventMeta,
    pub mission_id: Option<Uuid>,
    pub kind: CognitiveEventKind,
}

/// Pluggable history backend (Vec today, SQLite/RocksDB later).
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, event: &CognitiveEvent);
    async fn recent(&self, n: usize) -> Vec<CognitiveEvent>;
}

#[derive(Default)]
pub struct InMemoryEventStore {
    events: RwLock<Vec<CognitiveEvent>>,
    max: usize,
}

impl InMemoryEventStore {
    pub fn new(max: usize) -> Self {
        Self { events: RwLock::new(Vec::new()), max }
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append(&self, event: &CognitiveEvent) {
        let mut guard = self.events.write();
        guard.push(event.clone());
        if guard.len() > self.max {
            let excess = guard.len() - self.max;
            guard.drain(0..excess);
        }
    }

    async fn recent(&self, n: usize) -> Vec<CognitiveEvent> {
        let guard = self.events.read();
        guard.iter().rev().take(n).cloned().collect()
    }
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<CognitiveEvent>,
    store: Arc<dyn EventStore>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx, store: Arc::new(InMemoryEventStore::new(256)) }
    }

    pub fn with_store(store: Arc<dyn EventStore>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx, store }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CognitiveEvent> {
        self.tx.subscribe()
    }

    pub async fn publish(&self, event: CognitiveEvent) {
        self.store.append(&event).await;
        // Ignore lagging receivers – cognitive runtime must not block on slow listeners.
        let _ = self.tx.send(event);
    }

    pub async fn recent(&self, n: usize) -> Vec<CognitiveEvent> {
        self.store.recent(n).await
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
