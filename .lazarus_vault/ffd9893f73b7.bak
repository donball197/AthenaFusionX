use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::agent_protocol::AgentAction;
use crate::{executor, json_parser};
use serde_json::json;

static TASK_SLAB: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub async fn send_task(
    agent: Arc<crate::neural_agent::MicroAgent>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<crate::memory::MemoryPalace>>,
    tx: broadcast::Sender<String>,
) -> String {
    let directive = directive.trim();
    if directive.is_empty() {
        return "NO_DIRECTIVE".into();
    }

    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_LOCK_FAIL");
    let mut session_obs = String::from("SENSORS_INIT: Substrate Ready.\n");

    for turn in 1..=4 {
        // Context shield: prevent runaway memory growth
        if session_obs.len() > 2000 {
            session_obs = session_obs[session_obs.len() - 1000..].to_string();
        }

        let prompt = format!(
            "DIRECTIVE: {}\n\nOBSERVATIONS:\n{}\n\nExecute:",
            directive, session_obs
        );
        let ctx = crate::neural_agent::ContextPacket { ..Default::default() };

        match agent.prepare_and_run(client, api_key, &prompt, ctx, 0.7, None).await {
            Ok((response, _)) => {
                // Extract THOUGHT cleanly from the complete response
                if let (Some(ts), Some(te)) = (
                    response.find("<THOUGHT>"),
                    response.find("</THOUGHT>"),
                ) {
                    if te > ts + 9 {
                        let thought = response[ts + 9..te].trim().to_string();
                        if !thought.is_empty() {
                            let _ = tx.send(
                                json!({"type": "NEURAL_THOUGHT", "payload": thought})
                                    .to_string(),
                            );
                        }
                    }
                }

                match json_parser::extract_actions(&response) {
                    Ok(actions) => {
                        // ⚡ FIX: Empty action array [] means the model is done — treat as halt
                        if actions.is_empty() {
                            session_obs.push_str("SYSTEM: Task complete (no further actions).\n");
                            break;
                        }

                        // Check for halt before execution
                        let has_halt = actions.iter().any(|a| matches!(a, AgentAction::Halt));

                        match executor::Executor::execute_handle(
                            actions.clone(),
                            memory.clone(),
                        )
                        .await
                        {
                            Ok(results) => {
                                let combined = results.join("\n");
                                // Stream results to UI and CLI
                                let _ = tx.send(
                                    json!({
                                        "type": "EXECUTOR",
                                        "payload": combined
                                    })
                                    .to_string(),
                                );
                                let _ = tx.send(
                                    json!({
                                        "type": "NEURAL_STREAM",
                                        "payload": format!(
                                            "\n\x1b[32m>> [EXECUTOR Turn {}]:\n{}\x1b[0m\n",
                                            turn, combined
                                        )
                                    })
                                    .to_string(),
                                );
                                session_obs.push_str(&format!("RESULT: {}\n", combined));
                            }
                            Err(e) => {
                                let _ = tx.send(
                                    json!({
                                        "type": "NEURAL_STREAM",
                                        "payload": format!(
                                            "\n\x1b[31m>> [ERROR]: {}\x1b[0m\n", e
                                        )
                                    })
                                    .to_string(),
                                );
                                session_obs.push_str(&format!("ERROR: {}\n", e));
                            }
                        }

                        if has_halt {
                            break;
                        }
                    }
                    Err(_) => {
                        // No valid ACTION block — model may have answered in plain text, continue
                        session_obs.push_str("SYSTEM: No ACTION block found, continuing.\n");
                        continue;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(
                    json!({
                        "type": "NEURAL_STREAM",
                        "payload": format!("\n\x1b[31m>> [API_ERROR]: {}\x1b[0m\n", e)
                    })
                    .to_string(),
                );
                break;
            }
        }
    }

    "TASK_COMPLETE".into()
}
