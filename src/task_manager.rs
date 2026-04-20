use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::agent_protocol::AgentAction;
use crate::{executor, json_parser};
use serde_json::json;

static TASK_SLAB: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));

const MAX_TURNS: u32 = 12;

pub async fn send_task(
    agent: Arc<crate::neural_agent::MicroAgent>,
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
            session_obs = session_obs[session_obs.len() - 1500..].to_string();
        }

        // ⚡ AUTONOMIC REFLEX: Inject the current VMO handles directly into the observation buffer
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

        match agent.prepare_and_run(client, api_key, &prompt, ctx, 0.6, None).await {
            Ok((response, _)) => {
                // Extract thought for clean display
                if let (Some(ts), Some(te)) = (response.find("<THOUGHT>"), response.find("</THOUGHT>")) {
                    if te > ts + 9 {
                        let thought = response[ts + 9..te].trim().to_string();
                        if !thought.is_empty() {
                            let _ = tx.send(json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string());
                        }
                    }
                }

                // Use cleaned response for action parsing
                let clean_response = crate::neural_agent::strip_thought_regions(&response);

                match json_parser::extract_actions(&response) {
                    Ok(mut actions) => {
                        if actions.is_empty() {
                            session_obs.push_str("SYSTEM: No valid action found.\n");
                            continue;
                        }

                        // ENFORCE EXACTLY ONE ACTION PER TURN
                        if actions.len() > 1 {
                            actions.truncate(1);
                        }

                        let has_halt = actions.iter().any(|a| matches!(a, AgentAction::Halt));

                        match executor::Executor::execute_handle(actions.clone(), memory.clone(), vmo_state.clone()).await {
                            Ok(results) => {
                                let combined = results.join("\n");
                                let _ = tx.send(json!({"type": "EXECUTOR", "payload": combined}).to_string());
                                let _ = tx.send(json!({
                                    "type": "NEURAL_STREAM",
                                    "payload": format!("\n\x1b[32m>> [EXECUTOR Turn {}]:\n{}\x1b[0m\n", turn, combined)
                                }).to_string());
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
