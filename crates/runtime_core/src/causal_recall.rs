use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalRecall {
    pub pattern: String,
    pub previous_cause: String,
    pub lesson: String,
    pub confidence: f32,
}

impl CausalRecall {
    pub fn matches(&self, observation: &str) -> bool {
        observation.to_lowercase().contains(&self.pattern.to_lowercase())
    }

    pub fn recommendation(&self) -> String {
        format!("Previous lesson: {} (confidence {:.2})", self.lesson, self.confidence)
    }
}
