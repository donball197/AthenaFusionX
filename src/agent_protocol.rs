/**
 * Project Nexus Singularity - v1.9.10 (Unified Protocol)
 * Hardened Schema + Legacy Support for Arbiter/Agents
 */
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AgentTaskType {
    Refactor, Audit, SystemCheck, NeuralInference,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentDirective {
    pub id: Uuid,
    pub task_type: AgentTaskType,
    pub payload: String,
    pub priority: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AgentStatus {
    Idle, Thinking, Executing, Healing, Error(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentAction {
    SystemStatus { data: serde_json::Value },
    WriteFile { path: String, content: String },
    ExecuteCommand { command: String },
    Halt,
}
