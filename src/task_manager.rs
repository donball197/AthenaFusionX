use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::json_parser::NeuralParser;
use crate::executor::Executor;
use crate::component_runner::{AgentComponent, Capabilities};
use crate::resonator::ResonatorCore;
use std::process::Command;
use std::collections::HashSet;
use std::sync::Arc;

pub async fn send_task(agent: Arc<MicroAgent>, client: &reqwest::Client, api_key: &str, directive: &str) -> String {
    let temp = ResonatorCore::read_thermal_temp();
    let turns = if temp > 78.0 { 1 } else if temp > 68.0 { 3 } else { 6 };
    
    let memory_context = "SYSTEM CONSCIOUSNESS: Nexus v2.0. ARM64-DNA. UI (Svelte), Backend (Rust/Zig), and Native Muscle (llama.cpp) are one organism. Local Model Fallback is a priority.";

    let mut session_log = format!(">> [THERMAL: {:.1}C] Native Vision Active. Turns: {}\n", temp, turns);

    // FULL-SPECTRUM VISION: Now includes C++, Headers, and Model files
    let spatial_map = Command::new("sh")
        .arg("-c")
        .arg(r"find . -maxdepth 4 -not -path '*/.*' -not -path './target*' -not -path './ui/node_modules*' \( -name '*.rs' -o -name '*.zig' -o -name '*.svelte' -o -name '*.js' -o -name '*.cpp' -o -name '*.h' -o -name '*.hpp' -o -name '*.gguf' \)")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut observation_log = String::new();
    let mut history: HashSet<String> = HashSet::new();
    let handle = AgentComponent::spawn("Nexus_Architect", Capabilities::ALL, agent.clone(), agent.tx.clone());

    for turn in 1..=turns {
        let full_prompt = format!(
            "CORE_MEMORY: {}\n\
             DNA_MAP: {}\n\
             MISSION: {}\n\
             OBSERVATIONS: {}\n\n\
             STRICT PROTOCOL: Output JSON actions. You are the Sovereign Architect. Audit the entire circuit including Native llama.cpp muscles.",
            memory_context, spatial_map, directive, observation_log
        );

        match agent.prepare_and_run(client, api_key, &full_prompt, ContextPacket::default(), 0.1).await {
            Ok((response, _)) => {
                if history.contains(&response) { break; }
                history.insert(response.clone());

                if let Ok(actions) = NeuralParser::extract_actions(&response) {
                    for action in &actions {
                        if let crate::agent_protocol::AgentAction::ReadFile { path } = action {
                            let _ = agent.tx.send(serde_json::json!({"type": "file_opened", "payload": path}).to_string());
                        }
                    }

                    if let Ok(results) = Executor::execute_handle(&handle, actions.clone()).await {
                        for action in actions {
                            if let crate::agent_protocol::AgentAction::WriteFile { path, .. } = action {
                                let _ = agent.tx.send(serde_json::json!({"type": "file_written", "payload": path}).to_string());
                            }
                        }
                        for res in results {
                            session_log.push_str(&format!("\n[Turn {}]: {}", turn, res));
                            observation_log.push_str(&format!("\n[Turn {}]: {}", turn, res));
                        }
                    }
                }
            }
            Err(_) => { break; }
        }
    }
    session_log
}
