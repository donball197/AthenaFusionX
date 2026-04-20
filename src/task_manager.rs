// src/task_manager.rs — v3.1 SOVEREIGN CORTEX
// Zero-Noise, Deterministic FSM Wiring with Cumulative Memory Consolidation

use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::agent_protocol::AgentAction;
use crate::json_parser;
use crate::executor::{RoboticExecutor, FsmState};
use crate::factory::AgentFactory;
use crate::logic_engine::LogicEngine;
use serde_json::json;

static TASK_SLAB: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(2)));

const MAX_TURNS: u32 = 12;
const KEEP_HEAD_CHARS: usize = 512;
const TAIL_WINDOW_CHARS: usize = 1500;

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
    if directive.is_empty() {
        return "NO_DIRECTIVE".into();
    }

    let _permit = TASK_SLAB
        .clone()
        .acquire_owned()
        .await
        .expect("SLAB_LOCK_FAIL");

    let mut session_obs = String::from("SENSORS_INIT: Substrate Ready.\n");
    let mut fsm = RoboticExecutor::new();
    let mut error_count: usize = 0;
    
    // ⚡ UPGRADE: Cumulative tracker for the Memory Palace
    let mut full_result = String::new();

    for turn in 1..=MAX_TURNS {
        // ── FSM TERMINAL GUARD ───────────────────────────────────────────────
        if fsm.current_state == FsmState::Complete || fsm.current_state == FsmState::Fault {
            break;
        }

        // ── DEADLOCK GUARD ───────────────────────────────────────────────────
        if LogicEngine::detect_deadlock(&session_obs) {
            let _ = tx.send(
                json!({
                    "type": "NEURAL_STREAM",
                    "payload": "\n\x1b[31m>> [CORTEX] DEADLOCK DETECTED. Aborting mission.\x1b[0m\n"
                }).to_string(),
            );
            break;
        }

        // ── CONTEXT WINDOW TRIM ──────────────────────────────────────────────
        if session_obs.len() > KEEP_HEAD_CHARS + TAIL_WINDOW_CHARS {
            let head = session_obs[..KEEP_HEAD_CHARS].to_string();
            let tail_start = session_obs.len() - TAIL_WINDOW_CHARS;
            let tail = session_obs[tail_start..].to_string();
            session_obs = format!("{}\n[...trimmed...]\n{}", head, tail);
        }

        // ── VMO STATE INJECTION ──────────────────────────────────────────────
        let vmo_status = {
            let state = vmo_state.lock().await;
            let parent_str = state.last_parent.map(|h| format!("0x{:x}", h)).unwrap_or_else(|| "None".to_string());
            let child_str = state.last_child.map(|h| format!("0x{:x}", h)).unwrap_or_else(|| "None".to_string());
            format!("CURRENT_VMO_STATE: last_parent={}, last_child={}\n", parent_str, child_str)
        };

        // ── MODEL ESCALATION ─────────────────────────────────────────────────
        let escalated_model = AgentFactory::get_model_for_turn(turn as usize, error_count);

        if turn > 1 {
            let _ = tx.send(
                json!({
                    "type": "NEURAL_STREAM",
                    "payload": format!("\x1b[34m>> [CORTEX] Turn {} — Model: {}\x1b[0m\n", turn, escalated_model)
                }).to_string(),
            );
        }

        let prompt = format!(
            "DIRECTIVE: {}\n\nOBSERVATIONS:\n{}{}\n\nExecute:",
            directive, session_obs, vmo_status
        );

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: if turn >= 3 {
                Some(format!("You are NEXUS Ghost. Preferred model for this turn: {}.", escalated_model))
            } else {
                None
            },
            ..Default::default()
        };

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
                    Ok(actions) => {
                        if actions.is_empty() {
                            session_obs.push_str("SYSTEM: No valid action found.\n");
                            error_count += 1;
                            continue;
                        }

                        let has_halt = actions.iter().any(|a| matches!(a, AgentAction::Halt));

                        match fsm.execute_handle(actions, memory.clone(), vmo_state.clone()).await {
                            Ok(results) => {
                                let combined = results.join("\n");
                                
                                // ⚡ UPGRADE: Accumulate mission history
                                if !full_result.is_empty() { full_result.push_str("\n"); }
                                full_result.push_str(&format!("[Turn {}] {}", turn, combined));

                                let _ = tx.send(
                                    json!({
                                        "type": "NEURAL_STREAM",
                                        "payload": format!("\n\x1b[32m>> [EXECUTOR Turn {}]:\n{}\x1b[0m\n", turn, combined)
                                    }).to_string(),
                                );
                                session_obs.push_str(&format!("RESULT: {}\n", combined));
                                error_count = 0; 
                                
                                // ⚡ UPGRADE: Immediate Severance Check
                                if fsm.current_state == FsmState::Complete || fsm.current_state == FsmState::Fault {
                                    break; 
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(json!({"type": "NEURAL_STREAM", "payload": format!("\n\x1b[31m>> [ERROR]: {}\x1b[0m\n", e)}).to_string());
                                session_obs.push_str(&format!("ERROR: {}\n", e));
                                error_count += 1;
                            }
                        }

                        if has_halt { break; }
                    }
                    Err(_) => {
                        session_obs.push_str("SYSTEM: No ACTION block found. Continuing.\n");
                        error_count += 1;
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

    // ── MEMORY COMMIT ────────────────────────────────────────────────────────
    if !full_result.is_empty() {
        let palace = memory.lock().await;
        if palace.store_memory("TASK_MANAGER", directive, &full_result).await.is_ok() {
            let _ = tx.send(json!({"type": "NEURAL_STREAM", "payload": "\n\x1b[35m>> [MEMORY_PALACE] Mission trajectory enshrined.\x1b[0m\n"}).to_string());
        }
    }

    "TASK_COMPLETE".into()
}
