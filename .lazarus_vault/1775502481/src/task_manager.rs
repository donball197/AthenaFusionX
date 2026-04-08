use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::json_parser::NeuralParser;
use crate::executor::Executor;
use crate::component_runner::{AgentComponent, Capabilities};
use crate::resonator::ResonatorCore;
use crate::memory::MemoryPalace;
use crate::local_llm::LocalLLM;
use std::sync::Arc;
use tokio::sync::Mutex;
use regex::Regex;
use std::fs;
use serde_json::json;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn send_task(
    agent: Arc<MicroAgent>, 
    client: &reqwest::Client, 
    api_key: &str, 
    directive: &str,
    memory: Arc<Mutex<MemoryPalace>>
) -> String {
    let temp = ResonatorCore::read_thermal_temp();
    let turns = if temp > 78.0 { 1 } else if temp > 68.0 { 3 } else { 6 };
    
    let role = if directive.contains("NEXUS_ARCHITECT") { "NEXUS_ARCHITECT" }
               else if directive.contains("NEXUS_UI_DRONE") || (directive.to_lowercase().contains("ui") && !directive.contains("ARCHITECT")) { "NEXUS_UI_DRONE" }
               else if directive.contains("NEXUS_KERNEL_SENTINEL") || directive.to_lowercase().contains("kernel") { "NEXUS_KERNEL_SENTINEL" }
               else { "NEXUS_ARCHITECT" };

    let lower_directive = directive.to_lowercase();
    let mut target_dna = String::from("No explicit file DNA detected in directive.");

    if lower_directive.contains("audit") || lower_directive.contains("review") || lower_directive.contains("scan") {
        println!("\x1b[36m>> [CORTEX_INJECTOR] Broad analysis detected. Slurping local workspace DNA...\x1b[0m");
        let _ = agent.tx.send(String::from(">> [CORTEX_INJECTOR] Broad analysis detected. Slurping workspace DNA..."));
        
        let mut context = String::from("\n--- [SYSTEM WORKSPACE DNA] ---\n");
        if let Ok(cargo) = fs::read_to_string("Cargo.toml") {
            context.push_str(&format!("\n// === FILE: Cargo.toml ===\n{}\n", cargo));
        }
        if let Ok(entries) = fs::read_dir("src") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        context.push_str(&format!("\n// === FILE: {} ===\n{}\n", path.display(), content));
                    }
                }
            }
        }
        target_dna = context;
    } else {
        let path_regex = Regex::new(r"([a-zA-Z0-9_\-/]+\.[a-z]{1,4})").unwrap();
        if let Some(caps) = path_regex.captures(directive) {
            let path = &caps[0];
            if let Ok(content) = fs::read_to_string(path) {
                target_dna = format!("--- START FILE DNA: {} ---\n{}\n--- END FILE DNA ---", path, content);
                println!("\x1b[32m>> [SUTURE] Successfully injected DNA from {}\x1b[0m", path);
            }
        }
    }

    let is_complex = directive.contains("ARCHITECT") || directive.contains("REFACTOR") || directive.contains("SURGERY") || directive.contains("audit") || directive.contains("scan");
    
    if !is_complex || temp > 75.0 {
        println!("\x1b[33m>> [SWARM] Routing to Edge Cortex (Local Reflex). Thermal: {:.1}C\x1b[0m", temp);
        let edge_cortex = LocalLLM::new();
        
        if let Ok(local_resp) = edge_cortex.query_gemma_nano(directive, "vault/models/gemma-3n-e2b.tflite") {
            let mem_guard = memory.lock().await;
            let _ = mem_guard.store_memory(role, directive, &local_resp).await;
            
            let _ = agent.tx.send(json!({"type": "ghost_start"}).to_string());
            let _ = agent.tx.send(json!({"type": "ghost_token", "payload": local_resp}).to_string());
            let _ = agent.tx.send(json!({"type": "ghost_end"}).to_string());
            
            return local_resp;
        }
        println!("\x1b[31m!! [REFLEX_FAULT] Edge Cortex stalled. Escalating to 27B Uplink...\x1b[0m");
    }

    let sys_prompt = match role {
        "NEXUS_UI_DRONE" => "You are the UI specialist. Analyze Svelte/CSS DNA provided below.",
        "NEXUS_KERNEL_SENTINEL" => "You are the Kernel Auditor. Analyze the provided Rust/Zig DNA for alignment and safety.",
        _ => "You are the Lead Architect. Coordinate the swarm using the provided context."
    };

    println!("\x1b[35m>> [SWARM] Routing to {} (Gemma-4 26B Uplink). (Thermal: {:.1}C)\x1b[0m", role, temp);

    let mut session_log = format!(">> [{}] Active turns: {}\n", role, turns);
    let mut observation_log = String::new();

    for turn in 1..=turns {
        let full_prompt = format!(
            "SYSTEM_ROLE: {}\nTARGET_DNA: {}\nMISSION: {}\nOBSERVATIONS: {}\n\n\
            STRICT PROTOCOL: Respond ONLY with a JSON array of actions.",
            sys_prompt, target_dna, directive, observation_log
        );

        let _ = agent.tx.send(json!({"type": "ghost_start"}).to_string());

        // === SUTURE: NEURAL SPINNER ===
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} \x1b[36m>> [{msg}] SYNTHESIZING 262K DNA...\x1b[0m")
            .unwrap());
        spinner.set_message(role.to_string());
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        let run_result = agent.prepare_and_run(client, api_key, &full_prompt, ContextPacket::default(), 0.1).await;
        
        // Kill spinner when synthesis is complete
        spinner.finish_and_clear();

        match run_result {
            Ok((response, _)) => {
                let _ = agent.tx.send(json!({"type": "ghost_token", "payload": response.clone()}).to_string());
                let _ = agent.tx.send(json!({"type": "ghost_end"}).to_string());

                if let Err(e) = Executor::execute_swarm_json(&response, &agent.tx).await {
                    let msg = format!("!! [EXECUTION FAULT]: {}", e);
                    println!("\x1b[31m{}\x1b[0m", msg);
                    let _ = agent.tx.send(msg);
                }

                if let Ok(actions) = NeuralParser::extract_actions(&response) {
                    let handle = AgentComponent::spawn(role, Capabilities::ALL, agent.clone(), agent.tx.clone());
                    if let Ok(results) = Executor::execute_handle(&handle, actions).await {
                        for res in results {
                            let entry = format!("\n[Turn {}]: {}", turn, res);
                            session_log.push_str(&entry);
                            observation_log.push_str(&entry);
                        }
                    }
                } else {
                    session_log.push_str(&format!("\n[Turn {} Message]: {}", turn, response));
                    break; 
                }
            }
            Err(e) => {
                let _ = agent.tx.send(json!({"type": "ghost_end"}).to_string());
                let err_msg = format!("!! [Turn {} FAIL]: {}", turn, e);
                println!("\x1b[31m{}\x1b[0m", err_msg);
                let _ = agent.tx.send(err_msg.clone());
                session_log.push_str(&format!("\n{}", err_msg));
                break;
            }
        }
    }
    
    let re_thought = Regex::new(r"(?s)<\|channel>thought.*?<channel\|>").unwrap_or_else(|_| Regex::new("").unwrap());
    let clean_log = re_thought.replace_all(&session_log, "[REASONING_REDACTED]").to_string();
    let mem_guard = memory.lock().await;
    let _ = mem_guard.store_memory(role, directive, &clean_log).await;
    
    session_log
}
