use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::agent_protocol::AgentAction;
use crate::{executor, json_parser};
use serde_json::json;

static TASK_SLAB: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));

const MAX_TURNS: u32 = 12;

pub async fn send_task(
    orch: Arc<crate::orchestrator::Orchestrator>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    vmo_state: Arc<tokio::sync::Mutex<crate::server::VmoState>>,
    tx: broadcast::Sender<String>,
) -> String {
    let directive = directive.trim();
    if directive.is_empty() { return "NO_DIRECTIVE".into(); }

    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_LOCK_FAIL");
    let mut session_obs = String::from("SENSORS_INIT: Substrate Ready.\n");

    for turn in 1..=MAX_TURNS {
        if session_obs.len() > 3000 {
            let head = &session_obs[..512];
            let tail = &session_obs[session_obs.len() - 1500..];
            session_obs = format!("{}\n\n[...trimmed...]\n\n{}", head, tail);
        }

        let vmo_status = {
            let state = vmo_state.lock().await;
            let parent_str = state.last_parent.map(|h| format!("0x{:x}", h)).unwrap_or_else(|| "None".to_string());
            let child_str = state.last_child.map(|h| format!("0x{:x}", h)).unwrap_or_else(|| "None".to_string());
            format!("CURRENT_VMO_STATE: last_parent={}, last_child={}\n", parent_str, child_str)
        };

        let prompt = format!(
            "DIRECTIVE: {}\n\nOBSERVATIONS:\n{}{}\n\nExecute:",
            directive, session_obs, vmo_status
        );
        let ctx = crate::neural_agent::ContextPacket { ..Default::default() };

        match orch.run(prompt, ctx, client, api_key, Some(tx.clone())).await {
            Ok(response) => {
                if let (Some(ts), Some(te)) = (response.find("<THOUGHT>"), response.find("</THOUGHT>")) {
                    if te > ts + 9 {
                        let thought = response[ts + 9..te].trim().to_string();
                        if !thought.is_empty() {
                            let _ = tx.send(json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string());
                        }
                    }
                }

                match json_parser::extract_actions(&response) {
                    Ok(actions) => { // Removed 'mut' to fix warning
                        if actions.is_empty() {
                            session_obs.push_str("SYSTEM: No valid action found.\n");
                            continue;
                        }

                        let has_halt = actions.iter().any(|a| matches!(a, AgentAction::Halt));

                        let mut fsm = crate::executor::RoboticExecutor::new();
                        match fsm.execute_handle(actions.clone(), memory.clone(), vmo_state.clone()).await {
                            Ok(results) => {
                                let combined = results.join("\n");
                                let _ = tx.send(json!({"type": "NEURAL_STREAM", "payload": format!("\n\x1b[32m>> [EXECUTOR Turn {}]:\n{}\x1b[0m\n", turn, combined)}).to_string());
                                session_obs.push_str(&format!("RESULT: {}\n", combined));
                            }
                            Err(e) => {
                                let _ = tx.send(json!({"type": "NEURAL_STREAM", "payload": format!("\n\x1b[31m>> [ERROR]: {}\x1b[0m\n", e)}).to_string());
                                session_obs.push_str(&format!("ERROR: {}\n", e));
                            }
                        }

                        if has_halt { break; }
                    }
                    Err(_) => {
                        session_obs.push_str("SYSTEM: No ACTION block found. Continuing.\n");
                        continue;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(json!({"type": "NEURAL_STREAM", "payload": format!("\n\x1b[31m>> [API_ERROR]: {}\x1b[0m\n", e)}).to_string());
                break;
            }
        }
    }
    "TASK_COMPLETE".into()
}
