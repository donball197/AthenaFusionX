//! Attention System – Phase 8B v2

use super::event_bus::{CognitiveEvent, CognitiveEventKind, EventBus, EventMeta};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct AttentionSystem {
    /// Weighted salience map (future). Single focus for now.
    salience: Arc<RwLock<HashMap<String, f32>>>,
    focus: Arc<RwLock<Option<String>>>,
    bus: EventBus,
}

impl AttentionSystem {
    pub fn new(bus: EventBus) -> Self {
        Self {
            salience: Arc::new(RwLock::new(HashMap::new())),
            focus: Arc::new(RwLock::new(None)),
            bus,
        }
    }

    pub fn current_focus(&self) -> Option<String> {
        self.focus.read().clone()
    }

    pub async fn shift(&self, new_focus: &str) {
        *self.focus.write() = Some(new_focus.to_string());
        self.salience.write().insert(new_focus.to_string(), 1.0);
        self.bus
            .publish(CognitiveEvent {
                meta: EventMeta::new("attention"),
                mission_id: None,
                kind: CognitiveEventKind::AttentionShift {
                    focus: new_focus.to_string(),
                },
            })
            .await;
    }

    pub fn set_salience(&self, topic: &str, weight: f32) {
        self.salience.write().insert(topic.to_string(), weight.clamp(0.0, 1.0));
    }

    pub fn top_salient(&self, k: usize) -> Vec<(String, f32)> {
        let mut items: Vec<_> = self.salience.read().iter().map(|(k, v)| (k.clone(), *v)).collect();
        items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        items.into_iter().take(k).collect()
    }

    pub fn clear(&self) {
        *self.focus.write() = None;
    }
}
