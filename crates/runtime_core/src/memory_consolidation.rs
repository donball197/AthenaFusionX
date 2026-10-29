use crate::memory_types::{CognitiveMemory, MemoryKind};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MemoryConsolidator;

impl MemoryConsolidator {
    pub fn new() -> Self {
        Self
    }

    pub fn calculate_retention(memory: &CognitiveMemory) -> f32 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

        let age = (now - memory.created_at).max(0) as f32;

        let decay = (-age / 86400.0).exp();

        let score = memory.importance * memory.confidence * decay;

        score.clamp(0.0, 1.0)
    }

    pub fn should_consolidate(memory: &CognitiveMemory) -> bool {
        match memory.kind {
            MemoryKind::Failure | MemoryKind::Reflection | MemoryKind::Goal => true,

            _ => Self::calculate_retention(memory) > 0.5,
        }
    }
}

impl Default for MemoryConsolidator {
    fn default() -> Self {
        Self::new()
    }
}
