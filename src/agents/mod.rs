use crate::agent_protocol::{AgentDirective, AgentStatus};
use anyhow::Result;
use async_trait::async_trait;

pub mod data_agent;
pub mod pruner;
pub mod scout_agent;
pub mod scribe_agent;

#[async_trait]
pub trait NexusAgent: Send + Sync {
    fn name(&self) -> &'static str;
    async fn status(&self) -> AgentStatus;
    async fn execute(&self, directive: AgentDirective) -> Result<String>;
}

pub enum ExpertDomain {
    SecurityAuditor,
    CodeArchitect,
    DocumentationBot,
    QualityControl,
}

pub fn get_expert_prompt(domain: ExpertDomain) -> &'static str {
    match domain {
        ExpertDomain::SecurityAuditor   => "IDENTITY: NEXUS Security Expert. ROLE: Identify logic flaws and substrate vulnerabilities.",
        ExpertDomain::CodeArchitect     => "IDENTITY: NEXUS Lead Architect. ROLE: Write high-performance Rust/Zig code.",
        ExpertDomain::DocumentationBot  => "IDENTITY: NEXUS Archivist. ROLE: Generate technical markdown and architecture maps.",
        ExpertDomain::QualityControl    => "IDENTITY: NEXUS Auditor. ROLE: Ensure 100% test coverage and zero compiler warnings.",
    }
}
pub mod agentic_runtime;
pub mod shell_agent;
