//! Working Memory – Phase 8B v2
//! Partitioned: facts / observations / reasoning / hypotheses.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryPartition {
    Facts,
    Observations,
    Reasoning,
    Hypotheses,
}

#[derive(Clone, Default)]
pub struct WorkingMemory {
    partitions: Arc<RwLock<HashMap<MemoryPartition, HashMap<String, String>>>>,
}

impl WorkingMemory {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        for p in [
            MemoryPartition::Facts,
            MemoryPartition::Observations,
            MemoryPartition::Reasoning,
            MemoryPartition::Hypotheses,
        ] {
            map.insert(p, HashMap::new());
        }
        Self {
            partitions: Arc::new(RwLock::new(map)),
        }
    }

    pub fn store(&self, partition: MemoryPartition, key: &str, value: &str) {
        if let Some(slot) = self.partitions.write().get_mut(&partition) {
            slot.insert(key.to_string(), value.to_string());
        }
    }

    pub fn recall(&self, partition: MemoryPartition, key: &str) -> Option<String> {
        self.partitions
            .read()
            .get(&partition)
            .and_then(|s| s.get(key).cloned())
    }

    pub fn clear_partition(&self, partition: MemoryPartition) {
        if let Some(slot) = self.partitions.write().get_mut(&partition) {
            slot.clear();
        }
    }

    pub fn clear_all(&self) {
        for slot in self.partitions.write().values_mut() {
            slot.clear();
        }
    }

    pub fn snapshot(&self, partition: MemoryPartition) -> HashMap<String, String> {
        self.partitions
            .read()
            .get(&partition)
            .cloned()
            .unwrap_or_default()
    }
}
