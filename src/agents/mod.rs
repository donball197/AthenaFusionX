use anyhow::Result;
use async_trait::async_trait;
use crate::agent_protocol::{AgentDirective, AgentStatus};

pub mod data_agent;
pub mod scribe_agent;
pub mod scout_agent;

#[async_trait]
pub trait NexusAgent: Send + Sync {
    fn name(&self) -> &'static str;
    async fn status(&self) -> AgentStatus;
    async fn execute(&self, directive: AgentDirective) -> Result<String>;
}
