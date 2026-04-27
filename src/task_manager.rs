// src/task_manager.rs — v4.7 [UNIT_VARIANT_ALIGNMENT]
use crate::distributed_scheduler::DistributedScheduler;
use crate::executor::{FsmState, RoboticExecutor};
use crate::json_parser;
use crate::router::IntelligentRouter;
use crate::logger;
use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Mutex, Semaphore};
use tracing::{info, warn, error, instrument};

static TASK_SLAB: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

fn max_turns() -> usize {
    std::env::var("NEXUS_MAX_TURNS").ok().and_then(|v| v.parse().ok()).unwrap_or(8)
}

#[instrument(skip(orch, client, memory, vmo_state, tx, dist), fields(directive_len = directive.len()))]
pub async fn send_task(
    orch: Arc<crate::orchestrator::Orchestrator>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    vmo_state: Arc<Mutex<crate::server::VmoState>>,
    tx: broadcast::Sender<String>,
    dist: Arc<DistributedScheduler>,
) -> String {
    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_LOCK_FAIL");
    
    let mut session_obs: Vec<String> = Vec::new();
    let mut fsm = RoboticExecutor::new(api_key.to_string(), tx.clone());
    let mut turn_logs: Vec<String> = Vec::new();

    let profile = IntelligentRouter::route(directive);

    for turn in 1..=max_turns() {
        // FIX: Fault is now a unit variant, allowing direct comparison
        if fsm.current_state == FsmState::Complete || fsm.current_state == FsmState::Fault {
            break;
        }

        let prompt = IntelligentRouter::build_prompt(directive, &profile, &session_obs);

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some(profile.persona.clone()),
            model_override: Some(profile.primary_model.to_string()),
            recent_logs: turn_logs.clone(), 
            ..Default::default()
        };

        if turn == 1 {
            let _ = tx.send(serde_json::json!({
                "type": "NEURAL_THOUGHT",
                "payload": format!(">> [ROUTER] Mission classified as {:?}. Escalating to Cortex.", profile.class)
            }).to_string());
        }

        logger::begin_turn_capture();

        match orch.run(prompt, ctx, client, api_key, Some(tx.clone())).await {
            Ok(res) => {
                match json_parser::extract_actions(&res) {
                    Ok(actions) => {
                        let _ = fsm.execute_handle(actions, memory.clone(), vmo_state.clone(), dist.clone()).await;
                    }
                    Err(e) => {
                        warn!(error = %e, "Cortex outputted malformed JSON.");
                        session_obs.push(format!("SYSTEM_ERROR: Your output was not valid JSON. {}", e));
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Uplink failure.");
                break;
            }
        }

        turn_logs = logger::end_turn_capture();
        info!(turn = turn, log_count = turn_logs.len(), "Turn cycle complete. Logs buffered for next turn.");
    }
    
    "TASK_COMPLETE".into()
}
