use crate::memory_types::{CognitiveMemory, MemoryKind};

use uuid::Uuid;

pub struct ReflectionEngine;

impl ReflectionEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn reflect(&self, memories: &[CognitiveMemory]) -> Option<CognitiveMemory> {
        if memories.is_empty() {
            return None;
        }

        let mut lesson = String::from("Reflection summary:\n");

        for memory in memories.iter().take(5) {
            lesson.push_str(&format!("- {}\n", memory.content));
        }

        Some(CognitiveMemory {
            id: Uuid::new_v4(),

            kind: MemoryKind::Reflection,

            key: format!("reflection_{}", Uuid::new_v4()),

            content: lesson,

            source: "ReflectionEngine".into(),

            confidence: 0.7,

            importance: 0.8,

            created_at: chrono::Utc::now().timestamp(),

            tags: vec!["reflection".into(), "lesson".into()],
        })
    }
}

impl Default for ReflectionEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub fn classify_lesson(lesson: &str) -> crate::memory_semantic::SemanticMemory {
    let category = if lesson.contains("RAM") || lesson.contains("memory") {
        crate::memory_semantic::SemanticCategory::HardwareConstraint
    } else if lesson.contains("failed") {
        crate::memory_semantic::SemanticCategory::FailurePattern
    } else {
        crate::memory_semantic::SemanticCategory::GeneralKnowledge
    };

    crate::memory_semantic::SemanticMemory::new(category, lesson, 0.8)
}
