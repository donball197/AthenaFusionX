use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Success,
    Fracture,
    Vetoed,
}

/// The state payload passed between DAG nodes to prevent Context Amnesia.
/// Injected directly into the Gemma 3 API system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEnvelope {
    pub previous_node_id: Option<Uuid>,
    pub status: NodeStatus,
    pub stderr: Option<String>,          // Captures compiler/OS errors
    pub critic_feedback: Option<String>, // Captures exact veto reasoning
    pub modified_files: Vec<String>,     // Tracks mutations across nodes
}

impl Default for ContextEnvelope {
    fn default() -> Self {
        Self {
            previous_node_id: None,
            status: NodeStatus::Pending,
            stderr: None,
            critic_feedback: None,
            modified_files: Vec::new(),
        }
    }
}
