use crate::agent_runtime::engine::{ExecutableAgent, Task, TaskResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct WorkerAgent;

#[async_trait]
impl ExecutableAgent for WorkerAgent {
    async fn execute(&self, task: Task) -> Result<TaskResult> {
        let output = format!("Processed task via Native Rust Traits: {}", task.payload);
        Ok(TaskResult { id: task.id, output, success: true })
    }
}
