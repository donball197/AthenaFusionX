use std::sync::Arc;
use crate::server::AppState;
use crate::inquisitor::Inquisitor;
use crate::neural_agent::{ContextPacket};
use anyhow::{Context, Result};

pub async fn run_comprehensive_fix(state: Arc<AppState>, target: &str) -> Result<String> {
    println!(">> [HEALER] Attempting Autonomic Fix for: {}", target);

    let error_log = if target == "ui" {
        Inquisitor::audit_ui()?
    } else {
        Inquisitor::audit_backend()?
    };

    if error_log == "NOMINAL" {
        return Ok("No faults detected in target.".into());
    }

    let prompt = format!("Fix this error: {}.", error_log);
    let ctx = ContextPacket {
        system_prompt: Some("You are the NEXUS SURGEON. Return ONLY code.".into()),
        ..Default::default()
    };

    let (suggestion, _) = state.agent.prepare_and_run(&state.client, &state.api_key, &prompt, ctx, 0.5)
        .await
        .context("Neural handshake failed during surgical synthesis")?;
    
    Ok(format!(">> [HEALER] Recommended Surgery:\n{}", suggestion))
}
