use nexus_substrate::memory::MemoryPalace;
use crate::{
    agent_protocol::AgentAction, cli, json_parser, logic_engine::LogicEngine,
    metrics::METRICS, orchestrator::Orchestrator, executor::RoboticExecutor,
    server::VmoState, distributed_scheduler::DistributedScheduler,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error};

pub async fn send_task(
    orchestrator: Arc<Orchestrator>,
    client: &Client,
    api_key: &str,
    raw_directive: &str,
    memory: Arc<Mutex<MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) -> String {
    METRICS.inc_task();
    let mut session_obs: Vec<String> = vec![];
    let mut history: Vec<(String, String)> = vec![]; 
    let mut turn = 0usize;
    let mut consecutive_fractures = 0usize;

    let execution_law = cli::get_execution_law_v6().await;
    let mut executor = RoboticExecutor::new(api_key.to_string(), tx.clone());

    'mission: while turn < 12 {
        turn += 1;
        info!("=== Turn {} / 12 ===", turn);

        if LogicEngine::detect_deadlock(&session_obs) { break 'mission; }
        if consecutive_fractures >= 3 { break 'mission; }

        let mut full_prompt = String::new();
        for (q, a) in &history { full_prompt.push_str(&format!("USER: {}\nMODEL: {}\n", q, a)); }
        
        let turn_prompt = if turn == 1 { raw_directive.to_string() } 
                          else { format!("The last action resulted in: {}. What is the next logical step?", 
                                         session_obs.last().cloned().unwrap_or_else(|| "SUCCESS".into())) };
        full_prompt.push_str(&format!("USER: {}", turn_prompt));

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some(execution_law.clone()),
            recent_logs: session_obs.iter().rev().take(3).rev().cloned().collect(),
            ..Default::default()
        };

        let response = match orchestrator.run(full_prompt, ctx, client, api_key, Some(tx.clone())).await {
            Ok(resp) => resp,
            Err(e) => { error!("Uplink failed: {}", e); break 'mission; }
        };

        match json_parser::extract_actions(&response) {
            Ok(actions) => {
                if actions.is_empty() { break 'mission; }
                history.push((turn_prompt, response.clone()));
                consecutive_fractures = 0;
                match executor.execute_handle(actions.clone(), memory.clone(), vmo_state.clone(), dist.clone()).await {
                    Ok(observations) => { for obs in observations { session_obs.push(obs); } }
                    Err(e) => session_obs.push(format!("EXECUTOR_FAULT: {}", e)),
                }
                if actions.iter().any(|a| matches!(a, AgentAction::Halt { .. })) { break 'mission; }
            }
            Err(_) => {
                consecutive_fractures += 1;
                session_obs.push("[FORMAT_VIOLATION] Output ONLY a raw JSON array.".into());
            }
        }
    }
    "TASK_COMPLETE".to_string()
}
