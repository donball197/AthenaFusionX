/**
 * Copyright 2026 Donald Ball
 * Project Nexus Singularity - v1.9 Agent Registry
 */
use anyhow::Result;
use async_trait::async_trait;
use crate::agent_protocol::{AgentDirective, AgentStatus};

pub mod pruner;
// pub mod refactorer; // Future MoE Expert
// pub mod auditor;    // Future MoE Expert

#[async_trait]
pub trait NexusAgent: Send + Sync {
    fn name(&self) -> &'static str;
    async fn status(&self) -> AgentStatus;
    async fn execute(&self, directive: AgentDirective) -> Result<String>;
}

/// Expert Routing Table for Gemma 4 26B MoE
#[derive(Debug, Clone, Copy)]
pub enum ExpertDomain {
    Pruning,    // Expert 0-3
    Refactor,   // Expert 4-7
    Security,   // Expert 8-11
    Sovereign,  // Kernel Level
}

pub fn route_to_expert(domain: ExpertDomain) -> &'static str {
    match domain {
        ExpertDomain::Pruning => "pruner_agent_v3",
        ExpertDomain::Refactor => "codex_expert_alpha",
        ExpertDomain::Security => "void_walker_audit",
        ExpertDomain::Sovereign => "nexus_core_reflex",
    }
}
