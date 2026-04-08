// src/auto_healer.rs - Cleaned + HealerState Restored
use std::sync::Arc;
use crate::server::AppState;
use crate::inquisitor::Inquisitor;
use crate::neural_agent::ContextPacket;
use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticStatus { Nominal, Degraded, Critical }

pub struct HealerState { pub overall_status: DiagnosticStatus }

pub async fn run_comprehensive_fix(state: Arc<AppState>, target: &str) -> Result<String> {
    println!(">> [HEALER] Starting autonomic repair for: {}", target);

    let error_log = if target == "ui" {
        Inquisitor::audit_ui()?
    } else {
        Inquisitor::audit_backend()?
    };

    if error_log == "NOMINAL" {
        return Ok("No faults detected.".into());
    }

    let prompt = format!("Fix this error log:\n{}", error_log);
    let ctx = ContextPacket {
        system_prompt: Some("You are the NEXUS SURGEON. Return ONLY the fixed code.".into()),
        ..Default::default()
    };

    let (suggestion, _) = state.agent.prepare_and_run(
        &state.client, 
        &state.api_key, 
        &prompt, 
        ctx, 
        0.5
    ).await.context("Neural repair handshake failed")?;

    Ok(format!(">> [HEALER] Surgery recommendation:\n{}", suggestion))
}

pub async fn background_heal_loop(_state: Arc<crate::server::AppState>) {
    // TODO: Implement thermal-aware background healing loop
    println!(">> [HEALER] Background loop initialized (stub)");
}
