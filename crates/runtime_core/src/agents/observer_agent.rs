use crate::agents::LogicAgent;
use crate::agents::agentic_runtime::SubstrateContext;
use anyhow::Result;
use async_trait::async_trait;

pub struct ObserverAgent;

#[async_trait]
impl LogicAgent for ObserverAgent {
    fn name(&self) -> String { "ObserverAgent".to_string() }
    
    fn can_handle(&self, _command: &str) -> bool {
        false // Observes but doesn't execute by default
    }

    async fn execute(&self, command: &str, _ctx: &SubstrateContext) -> Result<String> {
        Ok(format!("[OBSERVER] Noted: {}", command))
    }
}
