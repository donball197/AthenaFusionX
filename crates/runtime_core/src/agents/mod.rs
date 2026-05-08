pub mod agentic_runtime;
pub mod scout_agent;
pub mod shell_agent;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait LogicAgent: Send + Sync {
    fn name(&self) -> String;
    fn can_handle(&self, command: &str) -> bool;
    async fn execute(
        &self,
        command: &str,
        ctx: &agentic_runtime::SubstrateContext,
    ) -> Result<String>;
}
pub mod v2_worker;

use crate::agent_protocol::{AgentDirective, AgentStatus};

/// NexusAgent: The base trait for classic directive-style agents.
/// Use LogicAgent for the LRL (Logic-Routing Layer) agentic system.
#[async_trait::async_trait]
pub trait NexusAgent: Send + Sync {
    fn name(&self) -> &'static str;
    async fn status(&self) -> AgentStatus;
    async fn execute(&self, directive: AgentDirective) -> anyhow::Result<String>;
}

pub mod data_agent;
pub mod pruner;
pub mod scribe_agent;
