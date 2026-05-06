use crate::agents::NexusAgent;
use crate::agent_protocol::{AgentDirective, AgentStatus};
use anyhow::Result;
use async_trait::async_trait;

pub struct ShellAgent;

impl ShellAgent {
    // ⚡ CONTEXT SHIELD: Truncates massive shell outputs before they poison the LLM context
    fn truncate_for_context(output: &str, max_chars: usize) -> String {
        if output.len() <= max_chars {
            output.to_string()
        } else {
            let keep = max_chars / 2;
            format!("{}...\n\n[TRUNCATED — {} chars total]\n\n...{}", 
                &output[..keep], 
                output.len(), 
                &output[output.len().saturating_sub(keep)..])
        }
    }
}

#[async_trait]
impl NexusAgent for ShellAgent {
    fn name(&self) -> &'static str { "SHELL-AGENT" }
    async fn status(&self) -> AgentStatus { AgentStatus::Idle }
    async fn execute(&self, directive: AgentDirective) -> Result<String> {
        match crate::podman::PodmanShield::execute(&directive.payload) {
            Ok(output) => {
                let raw_stdout = String::from_utf8_lossy(&output.stdout);
                let raw_stderr = String::from_utf8_lossy(&output.stderr);
                
                let stdout = Self::truncate_for_context(raw_stdout.trim(), 2000);
                let stderr = Self::truncate_for_context(raw_stderr.trim(), 1000);
                
                let status = if output.status.success() { "SUCCESS" } else { "FAILED" };
                Ok(format!("STDOUT:\n{}\nSTDERR:\n{}\nSTATUS: {}", stdout, stderr, status))
            }
            Err(e) => Err(anyhow::anyhow!("PodmanShield Error: {}", e)),
        }
    }
}
