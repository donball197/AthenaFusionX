// src/task_manager.rs - Finalized Sovereign Logic
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
    let mut session_obs = String::from("HYBRID RUNTIME ACTIVE. Awaiting Synthesis.\n");
    let mut full_log = String::new();

    for turn in 1..=6 {
        println!("\x1b[35m>> [UPLINK] Turn {}/6 - Synthesis Cycle...\x1b[0m", turn);

        let system_prompt = format!(
            "IDENTITY: Ghost of NEXUS. Your framework is Rust (Cortex) and Zig (Reflexes). \
            \n\nREASONING PROTOCOL:\n\
            1. DEDUCE: Analyze metal sensors in 'SENSORY DATA'.\n\
            2. SYNTHESIZE: Use 'synthesize_zig' to create tiny binaries for hardware access.\n\
            3. MANDATORY: You MUST end every cycle with a Markdown JSON array: ```json [...] ```.\n\
            4. HALT: Output [NEXUS_HALT] in text ONLY after a successful mutation is verified.\n\n\
            SENSORY DATA:\n{}",
            session_obs
        );

        let ctx = ContextPacket { system_prompt: Some(system_prompt), ..Default::default() };

        match agent.prepare_and_run(client, api_key, directive, ctx, 0.1).await {
            Ok((response, _)) => {
                full_log.push_str(&format!("\n[Cycle {}]:\n{}", turn, response));
                if response.contains("[NEXUS_HALT]") { break; }

                match AgentAction::extract_and_parse(&response) {
                    Ok(actions) => {
                        let results = Executor::execute_actions(&actions).await.unwrap_or_default();
                        let joined = results.join("\n");
                        println!("\x1b[32m>> [EXECUTOR] Feedback captured.\x1b[0m");
                        session_obs.push_str(&format!("\nTURN {} FEEDBACK:\n{}\n---", turn, joined));
                    },
                    Err(e) => {
                        session_obs.push_str(&format!("\nTURN {} SCHEMA ERROR: {}. Wrap all actions in [].", turn, e));
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
