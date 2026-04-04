use std::sync::Arc;
use crate::server::AppState;
use crate::inquisitor::Inquisitor;
use crate::neural_agent::{ContextPacket};
use crate::nexus_error::NexusError;

#[derive(Debug, Clone)]
pub enum DiagnosticStatus { Nominal, Degraded, Critical }

pub struct HealerState { pub overall_status: DiagnosticStatus }

pub async fn run_comprehensive_fix(state: Arc<AppState>, target: &str) -> Result<String, String> {
    println!(">> [HEALER] Attempting Autonomic Fix for: {}", target);

    let error_log = if target == "ui" {
        Inquisitor::audit_ui().map_err(|e| e.to_string())?
    } else {
        Inquisitor::audit_backend().map_err(|e| e.to_string())?
    };

    if error_log == "NOMINAL" {
        return Ok("No faults detected in target.".into());
    }

    let prompt = format!("Fix this error: {}.", error_log);
    let ctx = ContextPacket {
        system_prompt: Some("You are the NEXUS SURGEON. Return ONLY code.".into()),
        ..Default::default()
    };

    // FIX: Added explicit type annotation for the neural handshake
    let result: Result<(String, u128), NexusError> = state.agent.prepare_and_run(&state.client, &state.api_key, &prompt, ctx, 0.5).await;
    
    match result {
        Ok((suggestion, _)) => Ok(format!(">> [HEALER] Recommended Surgery:\n{}", suggestion)),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn background_heal_loop(_state: Arc<crate::server::AppState>) {
    // Background loop implementation
}
