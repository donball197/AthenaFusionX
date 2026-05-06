import os

fpath = "src/task_manager.rs"
content = r"""use crate::{
    agent_protocol::AgentAction, cli, json_parser,
    orchestrator::Orchestrator, memory::MemoryPalace, server::VmoState,
    distributed_scheduler::DistributedScheduler
};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, error, warn};

pub async fn send_task(
    orchestrator: Arc<Orchestrator>, client: &Client, api_key: &str, raw_directive: &str,
    _memory: Arc<Mutex<MemoryPalace>>, _vmo_state: Arc<Mutex<VmoState>>,
    tx: broadcast::Sender<String>, _dist: Arc<DistributedScheduler>,
) -> String {
    let mut session_obs = vec![];
    let mut turn = 0;
    const MAX_TURNS: usize = 12; // ⚡ Increased headroom
    let directive = cli::inject_workspace_dna(raw_directive).await;

    while turn < MAX_TURNS {
        turn += 1;
        info!("=== Turn {} started ===", turn);

        let ctx = crate::neural_agent::ContextPacket {
            system_prompt: Some("You are NEXUS, a precise autonomous coding agent. Follow the execution law strictly.".to_string()),
            model_override: Some("gemma-4-26b-a4b-it".to_string()),
            recent_logs: session_obs.clone(), // ⚡ Memory linked
            ..Default::default()
        };

        let result = orchestrator.run(directive.clone(), ctx, client, api_key, Some(tx.clone())).await;

        let response = match result {
            Ok(resp) => {
                // ⚡ Cognitive Transparency: Log the raw stream
                info!("\x1b[34m[THOUGHT_STREAM] >>\x1b[0m\n{} ", resp);
                resp
            },
            Err(e) => {
                error!("Orchestrator uplink failed: {}", e);
                session_obs.push(format!("UPLINK_ERROR: {}", e));
                break;
            }
        };

        match json_parser::extract_actions(&response) {
            Ok(actions) if !actions.is_empty() => {
                let mut halted = false;
                for (i, action) in actions.iter().enumerate() {
                    // ⚡ Explicit Reasoning Capture
                    if let AgentAction::Halt { reasoning } = action {
                        info!("\x1b[35m[REASONING] >> {}\x1b[0m", reasoning);
                        halted = true;
                    } else {
                        // Extract reasoning from other action variants if possible
                        info!("Executing action {}/{}: {:?}", i + 1, actions.len(), action);
                    }

                    if halted { break; }

                    match crate::hitl::check_interception(action) {
                        Ok(None) => {
                            let obs_json = crate::shell::NexusShell::eval(action).await;
                            session_obs.push(obs_json.to_string());

                            if let AgentAction::WriteFile { .. } = action {
                                let check = AgentAction::ExecuteCommand {
                                    command: "cargo check".to_string(),
                                    reasoning: "Mandatory verification".to_string(),
                                };
                                session_obs.push(crate::shell::NexusShell::eval(&check).await.to_string());
                            }
                        }
                        Ok(Some(modified_directive)) => {
                            session_obs.push(serde_json::json!({"status": "modified_by_user", "directive": modified_directive}).to_string());
                        }
                        Err(feedback) => {
                            session_obs.push(serde_json::json!({"status": "blocked_by_user", "reason": feedback}).to_string());
                        }
                    }
                }
                if halted { 
                    info!("\x1b[32m>> [TERMINATION] Mission objectives met. Halting matrix.\x1b[0m");
                    break; 
                }
            }
            Ok(_) => {
                info!(">> [IDLE] Agent returned empty actions. Mission complete.");
                break;
            },
            Err(e) => {
                warn!("Parser failure: {}", e);
                session_obs.push(format!("PARSER_FRACTURE: {}", e));
                if turn >= MAX_TURNS - 2 { break; }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
    }

    if session_obs.iter().any(|o| o.contains("PARSER_FRACTURE") || o.contains("UPLINK_ERROR")) { "TASK_FAULTED".to_string() } else { "TASK_COMPLETE".to_string() }
}
"""
with open(fpath, "w") as f:
    f.write(content)
print("\x1b[32m>> [SUTURE] Absolute Sovereign Loop locked in task_manager.rs\x1b[0m")
