use crate::agents::{agentic_runtime::SubstrateContext, LogicAgent};
use anyhow::Result;
use async_trait::async_trait;

pub struct ShellAgent;

#[async_trait]
impl LogicAgent for ShellAgent {
    fn name(&self) -> String {
        "ShellAgent".to_string()
    }

    fn can_handle(&self, _command: &str) -> bool {
        true // Catch-all fallback
    }

    async fn execute(&self, command: &str, _ctx: &SubstrateContext) -> Result<String> {
        let out = crate::podman::PodmanShield::execute(command)?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}
