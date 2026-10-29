use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryKind {
    Observation,
    Experience,
    Goal,
    Reflection,
    Knowledge,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveMemory {
    pub id: Uuid,

    pub kind: MemoryKind,

    pub key: String,

    pub content: String,

    pub source: String,

    pub confidence: f32,

    pub importance: f32,

    pub created_at: i64,

    pub tags: Vec<String>,
}
