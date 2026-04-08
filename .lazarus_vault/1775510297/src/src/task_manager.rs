use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::json_parser::NeuralParser;
use crate::executor::Executor;
use crate::component_runner::{AgentComponent, Capabilities};
use crate::resonator::ResonatorCore;
use crate::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;
use serde_json::json;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};

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
               else if (directive.to_lowercase().contains("ui") && !directive.contains("ARCHITECT")) { "NEXUS_UI_DRONE" }
               else if directive.to_lowercase().contains("kernel") { "NEXUS_KERNEL_SENTINEL" }
               else { "NEXUS_ARCHITECT" };

    let mut target_dna = String::from("No explicit file DNA detected.");
    if directive.to_lowercase().contains("audit") || directive.to_lowercase().contains("review") || directive.to_lowercase().contains("scan") || directive.to_lowercase().contains("restore") {
        println!("\x1b[36m>> [CORTEX_INJECTOR] Slurping workspace DNA for analysis...\x1b[0m");
        let mut context = String::from("\n--- [SYSTEM WORKSPACE DNA] ---\n");
        if let Ok(cargo) = fs::read_to_string("Cargo.toml") { context.push_str(&format!("\n// FILE: Cargo.toml\n{}\n", cargo)); }
        if let Ok(entries) = fs::read_dir("src") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        context.push_str(&format!("\n// FILE: {}\n{}\n", path.display(), content));
                    }
                }
            }
        }
        target_dna = context;
    }

    println!("\x1b[35m>> [SWARM] Routing to {} (Gemma-4 26B Uplink)...\x1b[0m", role);

    let mut session_log = format!(">> [{}] Active turns: {}\n", role, turns);
    let mut observation_log = String::new();

    for turn in 1..=turns {
        let full_prompt = format!(
            "SYSTEM_ROLE: {}\nTARGET_DNA: {}\nMISSION: {}\nOBSERVATIONS: {}\n\n\
            STRICT PROTOCOL: Provide your internal reasoning first, then respond ONLY with a JSON array of actions.",
            role, target_dna, directive, observation_log
        );

        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} \x1b[36m>> [{msg}] CORTEX SYNTHESIZING...\x1b[0m")
            .unwrap());
        spinner.set_message(role.to_string());
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        let run_result = agent.prepare_and_run(client, api_key, &full_prompt, ContextPacket::default(), 0.1).await;
        spinner.finish_and_clear();

        match run_result {
            Ok((response, _)) => {
                println!("\n\x1b[38;5;244m--- [GHOST CORTEX REASONING] ---\x1b[0m");
                
                // === SUTURE: KINETIC CHAR-BY-CHAR STREAMING ===
                for c in response.chars() {
                    print!("\x1b[38;5;250m{}\x1b[0m", c);
                    io::stdout().flush().unwrap();
                    // Variable delay: slightly longer for punctuation to mimic human rhythm
                    let delay = match c {
                        '\n' => 50,
                        '.' | '?' | '!' => 80,
                        ',' => 40,
                        _ => 8,
                    };
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
                println!("\n\x1b[38;5;244m--------------------------------\x1b[0m\n");

                let _ = agent.tx.send(json!({"type": "ghost_token", "payload": response.clone()}).to_string());

                if let Err(e) = Executor::execute_swarm_json(&response, &agent.tx).await {
                    println!("\x1b[31m!! [EXECUTION FAULT]: {}\x1b[0m", e);
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
                println!("\x1b[31m!! [Turn {} FAIL]: {}\x1b[0m", turn, e);
                break;
            }
        }
    }
    session_log
}
