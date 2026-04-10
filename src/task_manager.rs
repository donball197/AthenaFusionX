/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


// src/task_manager.rs - v5.2.0 (Explicit Feedback & Schema Lock)
use std::sync::{Arc, LazyLock};
use tokio::sync::{broadcast, mpsc, Semaphore, Mutex};
use crate::agent_protocol::AgentAction;
use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::memory::MemoryPalace;
use crate::{executor, json_parser};
use crate::component_runner::{AgentHandle, Capabilities};

static TASK_SLAB: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len { output.to_string() } 
    else { format!("{}...\n\n[TRUNCATED]", &output[..max_len]) }
}

pub async fn send_task(
    agent: Arc<MicroAgent>,
    client: &reqwest::Client,
    api_key: &str,
    directive: &str,
    memory: Arc<Mutex<MemoryPalace>>,
    tx: broadcast::Sender<String>,
) -> String {
    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_FAIL");
    let mut session_obs = String::from("SENSORS_INIT: Metal Ready.\n");
    let mut full_log = String::new();
    
    let is_complex = directive.len() > 60 || ["scrape", "analyze", "compare"].iter().any(|&w| directive.to_lowercase().contains(w));
    let mut resonance: f32 = if is_complex { 0.8 } else { 0.2 };

    let (tx_internal, _rx) = mpsc::channel(8);
    let handle = AgentHandle { tx: tx_internal, id: "internal".into(), capabilities: Capabilities::ALL };

    for turn in 1..=10 {
        let thinking_instruction = if resonance >= 0.7 {
            "1. Reason briefly in <THOUGHT> tags (MAX 2 SENTENCES)."
        } else {
            "1. Skip <THOUGHT> tags. Output <ACTION> immediately."
        };

        println!("\n\x1b[35m>> [NEURAL_PULSE] Turn {}/10 - Resonance: {:.1}\x1b[0m", turn, resonance);

        let system_prompt = format!(
            "IDENTITY: Ghost of NEXUS.\n\
            PROTOCOL:\n\
            {}\n\
            2. Output EXACTLY ONE JSON array wrapped in <ACTION> tags.\n\
            3. CRITICAL: Use 'url' for scrape_target and 'path' for read_file. DO NOT MIX THEM.\n\n\
            SCHEMA REFERENCE:\n\
            - {{\"action\": \"scrape_target\", \"url\": \"https://...\"}}\n\
            - {{\"action\": \"read_file\", \"path\": \"src/...\"}}\n\
            - {{\"action\": \"halt\"}}",
            thinking_instruction
        );

        let prompt = format!("DIRECTIVE: {directive}\n\nOBSERVATIONS:\n{session_obs}\n\nExecute:");
        let ctx = ContextPacket { system_prompt: Some(system_prompt), ..Default::default() };

        match tokio::time::timeout(std::time::Duration::from_secs(90), agent.prepare_and_run(client, api_key, &prompt, ctx, resonance)).await {
            Ok(Ok((response, _))) => {
                let _ = tx.send(response.clone()); 
                full_log.push_str(&format!("\n[Turn {}]:\n{}", turn, response));

                match json_parser::extract_actions(&response) {
                    Ok(actions) => {
                        if actions.iter().any(|a| matches!(a, AgentAction::Halt)) {
                            println!("\x1b[1;32m>> [NEXUS_HALT] Objective secured.\x1b[0m");
                            break;
                        }
                        
                        match executor::Executor::execute_handle(&handle, actions, memory.clone()).await {
                            Ok(obs) => {
                                let safe_obs = truncate_output(&obs.join("\n"), 5000);
                                session_obs.push_str(&format!("RESULT: {}\n", safe_obs));
                            }
                            Err(e) => {
                                session_obs.push_str(&format!("ERROR: {}\n", e));
                            }
                        }
                    },
                    Err(e) => {
                        resonance = 0.9; // Spike resonance to help it read the error
                        session_obs.push_str(&format!("{}\n", e));
                    }
                }
            },
            _ => break,
        }
    }
    let _ = memory.lock().await.store_memory("NEXUS_ARCHITECT", directive, &full_log).await;
    full_log
}
