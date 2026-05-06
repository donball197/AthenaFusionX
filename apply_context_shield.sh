#!/usr/bin/env bash
# apply_context_shield.sh — Fix LRL Routing & Shell Truncation
set -euo pipefail

cd /home/donball197/nexus_singularity-dev || exit 1
echo "🛠️ Applying Context Shield and LRL Routing Fixes..."

# 1. Forge the Truncation-Enabled ShellAgent
cat << 'EOF' > src/agents/shell_agent.rs
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
EOF

# 2. Forge the Expanded Routing Heuristic in AgenticRuntime
cat << 'EOF' > src/agents/agentic_runtime.rs
use crate::agents::NexusAgent;
use crate::agent_protocol::{AgentDirective, AgentTaskType};
use crate::server::VmoState;
use crate::distributed_scheduler::DistributedScheduler;
use nexus_substrate::memory::MemoryPalace;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SubstrateContext {
    pub memory: Arc<Mutex<MemoryPalace>>,
    pub vmo_state: Arc<Mutex<VmoState>>,
    pub dist: Arc<DistributedScheduler>,
}

pub struct AgenticRuntime {
    pub agents: Vec<Box<dyn NexusAgent>>,
    pub ctx: SubstrateContext,
}

impl AgenticRuntime {
    pub fn new(ctx: SubstrateContext) -> Self {
        Self { agents: Vec::new(), ctx }
    }

    pub fn register_agent(&mut self, agent: Box<dyn NexusAgent>) {
        self.agents.push(agent);
    }

    pub async fn dispatch(&self, payload: &str) -> Result<String> {
        println!("\x1b[1;34m[LRL_RUNTIME] Dispatching Agentic Pulse: {}\x1b[0m", payload);
        let mut logs = Vec::new();

        // ⚡ EXTENDED LRL ROUTING HEURISTIC
        let payload_lower = payload.to_lowercase();
        let is_shell = payload_lower.starts_with("cargo") 
            || payload_lower.starts_with("ls") 
            || payload_lower.starts_with("cat") 
            || payload_lower.starts_with("find") 
            || payload_lower.starts_with("git")
            || payload_lower.starts_with("echo")
            || payload_lower.starts_with("pwd")
            || payload_lower.starts_with("rm")
            || payload_lower.starts_with("mkdir")
            || payload_lower.starts_with("cp")
            || payload_lower.starts_with("mv")
            || payload_lower.starts_with("bash")
            || payload_lower.starts_with("sh");

        for agent in &self.agents {
            let name = agent.name();
            if is_shell && name != "SHELL-AGENT" { continue; }
            if !is_shell && name == "SHELL-AGENT" { continue; }

            let directive = AgentDirective {
                id: uuid::Uuid::new_v4(),
                task_type: AgentTaskType::Command,
                payload: payload.to_string(),
                priority: 1,
            };

            match agent.execute(directive).await {
                Ok(report) => {
                    println!("\x1b[1;32m[{}] Report: {}\x1b[0m", name, report);
                    logs.push(report);
                }
                Err(e) => {
                    eprintln!("\x1b[31m[{}] Execution Failure: {}\x1b[0m", name, e);
                    logs.push(format!("FAULT: {}", e));
                }
            }
        }
        Ok(logs.join("\n"))
    }
}
EOF

# 3. Silence the diagnostic warning in manual_trap.rs without deleting it
if [ -f src/manual_trap.rs ]; then
    if ! grep -q "#\[allow(dead_code)\]" src/manual_trap.rs; then
        sed -i '1i #\[allow(dead_code)\]' src/manual_trap.rs
    fi
fi

echo "🚀 Building hardened substrate..."
cargo build -j 1 --bin nexus

echo "✅ Truncation Shield & Routing Fixes Applied."
