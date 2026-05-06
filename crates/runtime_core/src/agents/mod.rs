pub mod agentic_runtime;
pub mod scout_agent;
pub mod shell_agent;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait LogicAgent: Send + Sync {
    fn name(&self) -> String;
    fn can_handle(&self, command: &str) -> bool;
    async fn execute(&self, command: &str, ctx: &agentic_runtime::SubstrateContext) -> Result<String>;
}
pub mod v2_worker;
