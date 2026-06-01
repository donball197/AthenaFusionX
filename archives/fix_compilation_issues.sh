#!/usr/bin/env bash
set -euo pipefail

echo "🔧 Fixing runtime_core compilation issues..."

# === Fix agents module structure ===
cat > crates/runtime_core/src/agents/mod.rs << 'RUST'
pub mod agentic_runtime;
pub mod scout_agent;
pub mod shell_agent;

// Re-export for clean imports
pub use agentic_runtime::SubstrateContext;
pub use crate::agents::LogicAgent;  // defined below

#[async_trait::async_trait]
pub trait LogicAgent: Send + Sync {
    fn name(&self) -> String;
    fn can_handle(&self, command: &str) -> bool;
    async fn execute(&self, command: &str, ctx: &SubstrateContext) -> anyhow::Result<String>;
}
RUST

# === Fix scout_agent ===
cat > crates/runtime_core/src/agents/scout_agent.rs << 'RUST'
use crate::agents::{LogicAgent, SubstrateContext};
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
RUST

# === Fix shell_agent ===
cat > crates/runtime_core/src/agents/shell_agent.rs << 'RUST'
use crate::agents::{LogicAgent, SubstrateContext};
use anyhow::Result;
use async_trait::async_trait;

pub struct ShellAgent;

#[async_trait]
impl LogicAgent for ShellAgent {
    fn name(&self) -> String { "ShellAgent".to_string() }
    
    fn can_handle(&self, _command: &str) -> bool { true }

    async fn execute(&self, command: &str, _ctx: &SubstrateContext) -> Result<String> {
        let out = crate::podman::PodmanShield::execute(command)?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}
RUST

# === Fix task_manager.rs (missing import + api_key) ===
sed -i '/use crate::executor::RoboticExecutor;/d' crates/runtime_core/src/task_manager.rs 2>/dev/null || true

cat > /tmp/task_manager_fix.rs << 'RUST'
use crate::executor::RoboticExecutor;
RUST

# Insert import after existing uses
sed -i '/use nexus_runtime_core::shared_state::VmoState;/r /tmp/task_manager_fix.rs' crates/runtime_core/src/task_manager.rs

# Fix api_key reference (use parameter from function)
sed -i 's|RoboticExecutor::new(api_key.to_string()|RoboticExecutor::new(api_key.to_string()|' crates/runtime_core/src/task_manager.rs

echo "✅ Core fixes applied"
