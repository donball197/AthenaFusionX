use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SemanticCategory {
    HardwareConstraint,
    Strategy,
    FailurePattern,
    Optimization,
    Policy,
    GeneralKnowledge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemory {
    pub category: SemanticCategory,
    pub lesson: String,
    pub confidence: f32,
    pub importance: f32,
    pub tags: Vec<String>,
}

impl SemanticMemory {
    pub fn new(category: SemanticCategory, lesson: impl Into<String>, confidence: f32) -> Self {
        Self {
            category,
            lesson: lesson.into(),
            confidence,
            importance: confidence,
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}
