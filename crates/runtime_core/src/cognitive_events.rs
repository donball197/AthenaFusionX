use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CognitiveEvent {
    IntentCreated { id: Uuid, objective: String },

    ActionExecuted { intent_id: Uuid, success: bool },

    VerificationCompleted { intent_id: Uuid, confidence: f32 },
}
