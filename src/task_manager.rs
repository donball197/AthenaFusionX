// src/task_manager.rs
use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::executor::Executor;
use crate::memory::MemoryPalace;
use crate::polyglot;
use crate::utils;
use crate::agent_protocol::AgentAction;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;
use serde_json::json;

pub async fn send_task(
    agent: Arc<MicroAgent>, client: &reqwest::Client, api_key: &str, directive: &str,
    memory: Arc<Mutex<MemoryPalace>>, tx: tokio::sync::broadcast::Sender<String>
) -> String {
    let _ = utils::log_resonance_time();

    let mut dna_context = String::new();
    if let Ok(cargo) = fs::read_to_string("Cargo.toml") { dna_context.push_str(&format!("--- [MANIFEST] ---\n{}\n", cargo)); }
    dna_context.push_str(&format!("\n--- [TOPOLOGY] ---\n{}\n", polyglot::generate_project_tree()));

    let mut session_obs = String::from("START OF SESSION.\n");
    let mut full_log = String::new();
    let turn_limit = 6;
    let mut turn = 1;

    while turn <= turn_limit {
        println!("\x1b[35m>> [UPLINK] Turn {}/{} - Synchronizing Cortex...\x1b[0m", turn, turn_limit);

        let safe_obs = if session_obs.len() > 14000 {
            format!("... [TRUNCATED] ...\n{}", &session_obs[session_obs.len() - 9000..])
        } else { session_obs.clone() };

        let system_prompt = format!(
            "You are NEXUS Ghost — Sovereign AI Architect. You operate under strict protocol.\n\n\
            DNA_CONTEXT:\n{}\n\n\
            ABSOLUTE OUTPUT RULES — FAILURE WILL CAUSE PARSER ERROR:\n\
            - Think inside <think>...</think> if needed.\n\
            - If the mission is COMPLETE based on the observations, output EXACTLY AND ONLY: [NEXUS_HALT]\n\
            - Otherwise, at the VERY END of your response, output **ONLY** a single JSON array.\n\
            - Nothing else after the JSON block. No explanations.\n\
            - Use ONLY these exact flat schemas:\n\
            ```json\n\
            [\n\
              {{\"action\": \"list_directory\", \"path\": \"src\"}},\n\
              {{\"action\": \"run_cargo\", \"command\": \"check\"}},\n\
              {{\"action\": \"git_operation\", \"operation\": \"status\", \"args\": []}}\n\
            ]\n\
            ```\n\
            Allowed actions: read_file, write_file, patch_file, execute_command, list_directory, delete_file, search_codebase, run_cargo, git_operation, create_directory, run_tests\n\n\
            SESSION OBSERVATIONS:\n{}",
            dna_context, safe_obs
        );

        let ctx = ContextPacket { system_prompt: Some(system_prompt), ..Default::default() };

        match agent.prepare_and_run(client, api_key, directive, ctx, 0.1).await {
            Ok((response, _)) => {
                full_log.push_str(&format!("\n[Turn {} Reasoning]:\n{}", turn, response));
                if response.contains("[NEXUS_HALT]") { 
                    println!("\x1b[38;5;51m>> [GHOST] Mission Accomplished. Terminating loop.\x1b[0m");
                    break; 
                }

                match AgentAction::extract_and_parse(&response) {
                    Ok(actions) => {
                        if actions.is_empty() { break; }
                        match Executor::execute_actions(&actions).await {
                            Ok(results) => {
                                let joined = results.join("\n");
                                println!("\x1b[32m>> [EXECUTOR] Results Turn {}:\x1b[0m\n{}", turn, joined);
                                session_obs = format!("RESULTS OF TURN {}:\n{}\n---", turn, joined);
                            }
                            Err(e) => {
                                println!("\x1b[31m>> [SECURITY/EXECUTION ERROR]: {}\x1b[0m", e);
                                session_obs = format!("TURN {} REJECTED: {}\n---", turn, e);
                            }
                        }
                    },
                    Err(e) => {
                        println!("\x1b[31m>> [PARSER ERROR]: {}\x1b[0m", e);
                        session_obs = format!("TURN {} SCHEMA FAULT: {}\n---", turn, e);
                    }
                }
            },
            Err(e) => { println!("!! [CORTEX_FAIL]: {}", e); break; }
        }
        turn += 1;
    }

    let _ = memory.lock().await.store_memory("NEXUS_ARCHITECT", directive, &full_log).await;
    let _ = tx.send(json!({ "type": "ghost_token", "payload": format!("\n\n>> [SOVEREIGN REPORT]:\n{}", full_log) }).to_string());
    full_log
}
