use crate::{
    agent_protocol::AgentAction, logic_engine::LogicEngine,
    orchestrator::Orchestrator, executor::RoboticExecutor,
    shared_state::VmoState, distributed_scheduler::DistributedScheduler,
    json_parser, cli_utils, metrics::METRICS,
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub async fn send_task(
    orchestrator: Arc<Orchestrator>,
    client: &Client,
    api_key: &str,
    raw_directive: &str,
    memory: Arc<Mutex<nexus_substrate::memory::MemoryPalace>>,
    vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) -> String {
    METRICS.inc_task();
    let mut session_obs = vec![];
    let mut turn = 0;
    let mut history: Vec<(String, String)> = vec![];
    let execution_law = cli_utils::get_execution_law_v6().await;
    let mut executor = RoboticExecutor::new(api_key.to_string(), tx.clone());

    'mission: while turn < 12 {
        turn += 1;
        if LogicEngine::detect_deadlock(&session_obs) { break; }
        
        let mut full_prompt = String::new();
        for (q, a) in &history { full_prompt.push_str(&format!("USER: {}\nMODEL: {}\n", q, a)); }
        
        let turn_prompt = if turn == 1 { raw_directive.to_string() } 
                          else { format!("Context: {}. Next step?", session_obs.last().cloned().unwrap_or_default()) };
        full_prompt.push_str(&format!("USER: {}", turn_prompt));

        let ctx = crate::neural_agent::ContextPacket { system_prompt: Some(execution_law.clone()), ..Default::default() };
        let response = match orchestrator.run(full_prompt, ctx, client, api_key, Some(tx.clone())).await {
            Ok(r) => r,
            Err(_) => break
        };

        if let Ok(actions) = json_parser::extract_actions(&response) {
            if actions.is_empty() { break; }
            history.push((turn_prompt, response.clone()));
            for action in actions {
                if let AgentAction::SpawnSubagent { ref instruction, tier, limit, .. } = action {
                    let _ = tx.send(serde_json::json!({"type":"TASK_STATUS","payload":"[RECURSION] Specialist Deployed"}).to_string());
                    let sub_ret = orchestrator.run_subagent(instruction.clone(), tier, limit, api_key).await;
                    session_obs.push(LogicEngine::linearize_handoff(&action, sub_ret));
                    continue 'mission; 
                }
                if matches!(action, AgentAction::Halt { .. }) { break 'mission; }
                if let Ok(obs) = executor.execute_handle(vec![action], memory.clone(), vmo_state.clone(), dist.clone()).await {
                    for o in obs { session_obs.push(o); }
                }
            }
        }
    }
    "TASK_COMPLETE".into()
}
