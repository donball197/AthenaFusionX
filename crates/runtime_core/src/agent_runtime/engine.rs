use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    pub output: String,
    pub success: bool,
}

#[async_trait]
pub trait ExecutableAgent: Send + Sync {
    async fn execute(&self, task: Task) -> Result<TaskResult>;
}
