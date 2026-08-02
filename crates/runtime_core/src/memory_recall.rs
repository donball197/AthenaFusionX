use serde::{Deserialize, Serialize};

use crate::memory_types::CognitiveMemory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallRequest {
    pub context: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResult {
    pub memories: Vec<CognitiveMemory>,
    pub confidence: f32,
}

pub struct MemoryRecallEngine;

impl MemoryRecallEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn rank(&self, memories: Vec<CognitiveMemory>, _context: &str) -> RecallResult {
        let count = memories.len();

        RecallResult { memories, confidence: if count > 0 { 0.8 } else { 0.0 } }
    }
}

impl Default for MemoryRecallEngine {
    fn default() -> Self {
        Self::new()
    }
}
