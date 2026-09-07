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

    /// Rank already-retrieved memories against the active context.
    ///
    /// The substrate remains responsible for semantic retrieval. This layer
    /// only provides deterministic secondary ordering using lexical overlap
    /// plus the stored confidence/importance metadata.
    ///
    /// Memory is advisory context and MUST NOT authorize completion, HALT,
    /// mutation, or policy decisions.
    pub fn rank(&self, memories: Vec<CognitiveMemory>, context: &str) -> RecallResult {
        if memories.is_empty() {
            return RecallResult { memories, confidence: 0.0 };
        }

        let context_terms = Self::terms(context);

        let mut scored: Vec<(f32, CognitiveMemory)> = memories
            .into_iter()
            .map(|memory| {
                let memory_text = format!(
                    "{} {} {} {}",
                    memory.key,
                    memory.content,
                    memory.source,
                    memory.tags.join(" ")
                );

                let memory_terms = Self::terms(&memory_text);

                let overlap =
                    context_terms.iter().filter(|term| memory_terms.contains(*term)).count();

                let lexical_relevance = if context_terms.is_empty() {
                    0.0
                } else {
                    overlap as f32 / context_terms.len() as f32
                };

                let score = (lexical_relevance * 0.60)
                    + (memory.confidence.clamp(0.0, 1.0) * 0.20)
                    + (memory.importance.clamp(0.0, 1.0) * 0.20);

                (score, memory)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let best_score = scored.first().map(|(score, _)| *score).unwrap_or(0.0);

        let memories = scored.into_iter().map(|(_, memory)| memory).collect();

        RecallResult { memories, confidence: best_score.clamp(0.0, 1.0) }
    }

    fn terms(text: &str) -> std::collections::HashSet<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '/')
            .filter(|term| term.len() >= 3)
            .map(ToString::to_string)
            .collect()
    }
}

impl Default for MemoryRecallEngine {
    fn default() -> Self {
        Self::new()
    }
}
