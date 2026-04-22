// src/task_manager.rs — v5.2 Observational Sovereignty
use std::sync::{Arc, LazyLock};
use std::process::Command;
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::json_parser;
use crate::executor::{RoboticExecutor, FsmState};
use crate::router::IntelligentRouter;
use crate::conductor::Conductor;
use serde_json::json;

static TASK_SLAB: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

fn get_current_branch() -> String {
    Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"]).output()
        .ok().and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "detached".into()).trim().to_string()
}

pub async fn send_task(
    orch:      Arc<crate::orchestrator::Orchestrator>,
    client:    &reqwest::Client,
    api_key:   &str,
    directive: &str,
    memory:    Arc<Mutex<crate::memory::MemoryPalace>>,
    vmo_state: Arc<Mutex<crate::server::VmoState>>,
    tx:        broadcast::Sender<String>,
) -> String {
    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_LOCK_FAIL");
    let profile = IntelligentRouter::route(directive);
    let mut session_obs: Vec<String> = Vec::new();
    let mut fsm = RoboticExecutor::new();

    for turn in 1..=profile.max_turns {
        if fsm.current_state == FsmState::Complete || fsm.current_state == FsmState::Fault { break; }

        let branch = get_current_branch();
        let prompt = IntelligentRouter::build_prompt(directive, &profile, &session_obs);

        let _ = tx.send(json!({
            "type": "NEURAL_THOUGHT",
            "payload": format!("── Turn {}/{} | Branch: {} ──", turn, profile.max_turns, branch)
        }).to_string());

        match Conductor::run_turn(&profile, prompt, turn as usize, &orch, client, api_key, Some(tx.clone())).await {
            Ok(response) => {
                match json_parser::extract_actions(&response) {
                    Ok(actions) if !actions.is_empty() => {
                        match fsm.execute_handle(actions, memory.clone(), vmo_state.clone()).await {
                            Ok(results) => {
                                for res in results { session_obs.push(res); }
                            }
                            Err(e) => session_obs.push(format!("Error: {}", e)),
                        }
                    }
                    _ => {
                        // ⚡ If no action tags, the model is likely speaking to you. End mission.
                        session_obs.push("System: Mission target likely achieved or clarification needed.".into());
                        break; 
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(json!({"type": "NEURAL_STREAM", "payload": format!("API_FAULT: {}", e)}).to_string());
                break;
            }
        }
    }
    "TASK_COMPLETE".into()
}
