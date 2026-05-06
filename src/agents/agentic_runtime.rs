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
        let shell_commands = ["cargo", "ls", "cat", "find", "git", "echo", "pwd", "rm", "mkdir", "cp", "mv", "bash", "sh", "grep", "tail", "head", "sed", "file"];
        let is_shell = shell_commands.iter().any(|&cmd| payload_lower.starts_with(cmd));

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
