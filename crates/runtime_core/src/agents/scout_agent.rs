use crate::agents::{LogicAgent, agentic_runtime::SubstrateContext};
use anyhow::Result;
use async_trait::async_trait;

pub struct ScoutAgent;

impl ScoutAgent {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl LogicAgent for ScoutAgent {
    fn name(&self) -> String { "ScoutAgent".to_string() }
    
    fn can_handle(&self, command: &str) -> bool {
        let c = command.to_lowercase();
        c.starts_with("ls") || c.starts_with("find") || c.starts_with("grep")
    }

    async fn execute(&self, command: &str, _ctx: &SubstrateContext) -> Result<String> {
        let out = crate::podman::PodmanShield::execute(command)?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}
