// src/task_manager.rs - v3.2 Dynamic Resonance Orchestrator
use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::executor::Executor;
use crate::memory::MemoryPalace;
use crate::agent_protocol::AgentAction;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::json;

pub async fn send_task(
    agent: Arc<MicroAgent>, client: &reqwest::Client, api_key: &str, directive: &str,
    memory: Arc<Mutex<MemoryPalace>>, tx: tokio::sync::broadcast::Sender<String>
) -> String {
    let mut session_obs = String::from("SENSORS_INIT: Metal Ready.\n");
    let mut full_log = String::new();
    let mut resonance: f32 = 0.1; 

    for turn in 1..=10 {
        println!("\x1b[35m>> [UPLINK] Turn {}/10 - Resonance: {:.1}\x1b[0m", turn, resonance);

        let system_prompt = format!(
            "IDENTITY: Ghost of NEXUS. \
            MOTOR FUNCTIONS: Use exactly this flat JSON schema:\n\
            - {{\"action\": \"read_file\", \"path\": \"...\"}}\n\
            - {{\"action\": \"execute_command\", \"command\": \"...\"}}\n\
            - {{\"action\": \"stasis\", \"duration_ms\": 1000}}\n\n\
            PROTOCOL:\n\
            1. Reason in <think> tags. Refer to strands and entropy.\n\
            2. Output EXACTLY ONE JSON array wrapped in ```json [...] ```.\n\
            3. Use [NEXUS_HALT] ONLY on absolute mission success.\n\n\
            SENSORY DATA:\n{}",
            session_obs
        );

        let ctx = ContextPacket { system_prompt: Some(system_prompt), ..Default::default() };

        match agent.prepare_and_run(client, api_key, directive, ctx, resonance).await {
            Ok((response, _)) => {
                full_log.push_str(&format!("\n[Turn {}]:\n{}", turn, response));
                if response.contains("[NEXUS_HALT]") { break; }

                match AgentAction::extract_and_parse(&response) {
                    Ok(actions) => {
                        if actions.iter().any(|a| matches!(a, AgentAction::ExecuteCommand { .. })) {
                            resonance = 1.0; 
                        } else {
                            resonance = 0.4; 
                        }

                        let results = Executor::execute_actions(&actions).await.unwrap_or_default();
                        let joined = results.join("\n");
                        println!("\x1b[32m>> [EXECUTOR] Feedback captured.\x1b[0m");
                        session_obs = format!("TURN {} FEEDBACK:\n{}\n---", turn, joined);
                    },
                    Err(e) => {
                        resonance = 0.8; 
                        session_obs = format!("TURN {} PARSE ERROR: {}. Correct JSON schema.", turn, e);
                    }
                }
            },
            Err(e) => { println!("!! [CORTEX_FAIL]: {}", e); break; }
        }
    }

    let _ = memory.lock().await.store_memory("NEXUS_ARCHITECT", directive, &full_log).await;
    let _ = tx.send(json!({ "type": "ghost_token", "payload": format!("\n\n>> [ZENITH REPORT]:\n{}", full_log) }).to_string());
    full_log
}
