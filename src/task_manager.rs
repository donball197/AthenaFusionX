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
    
    // THE ARBORETUM: Pulling full architectural context
    let memory_context = "SYSTEM: Nexus v2.0 [ARM64-DNA]. VISION: Full-Spectrum (Includes .env, .sh, and weights).";
    
    let mut session_log = format!(">> [THERMAL: {:.1}C] Zenith Vision Active. Turns: {}\n", temp, turns);

    // X-RAY SCAN: Including hidden files, models, and scripts. 
    // Excluding only heavy binaries/build artifacts to save context tokens.
    let spatial_map = Command::new("sh")
        .arg("-c")
        .arg(r"find . -maxdepth 4 -not -path './target*' -not -path './ui/node_modules*' -not -path './.git/*' \( -name '*.rs' -o -name '*.zig' -o -name '*.svelte' -o -name '*.js' -o -name '*.sh' -o -name '*.py' -o -name '*.gguf' -o -name '*.tflite' -o -name '.env' \)")
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
             STRICT PROTOCOL: JSON actions only. Output exactly [{{ \"action\": \"...\" }}]. \
             You can see everything. If you need to check secrets, read .env. \
             If you need to check deployment, read .sh files.",
            memory_context, spatial_map, directive, observation_log
        );

        match agent.prepare_and_run(client, api_key, &full_prompt, ContextPacket::default(), 0.1).await {
            Ok((response, _)) => {
                if history.contains(&response) { break; }
                history.insert(response.clone());

                if let Ok(actions) = NeuralParser::extract_actions(&response) {
                    for action in &actions {
                        // BROADCAST SIGNAL: Trigger UI tab switch on ReadFile
                        if let crate::agent_protocol::AgentAction::ReadFile { path } = action {
                            let _ = agent.tx.send(serde_json::json!({
                                "type": "file_opened", 
                                "payload": path
                            }).to_string());
                        }
                    }

                    if let Ok(results) = Executor::execute_handle(&handle, actions.clone()).await {
                        for res in results {
                            session_log.push_str(&format!("\n[Turn {}]: {}", turn, res));
                            observation_log.push_str(&format!("\n[Turn {}]: {}", turn, res));
                        }
                    }
                }
            }
            Err(_e) => break,
        }
    }
    session_log
}
