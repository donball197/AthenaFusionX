use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::executor::Executor;
use crate::memory::MemoryPalace;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;

pub async fn send_task(
    agent: Arc<MicroAgent>, 
    client: &reqwest::Client, 
    api_key: &str, 
    directive: &str,
    _memory: Arc<Mutex<MemoryPalace>>
) -> String {
    let mut dna_context = String::from("No initial DNA attached.");
    let low_dir = directive.to_lowercase();
    
    // Broadened DNA Slurper to include 'refactor' and 'review'
    if low_dir.contains("audit") || low_dir.contains("code") || low_dir.contains("fix") || 
       low_dir.contains("run") || low_dir.contains("refactor") || low_dir.contains("review") {
        println!("\x1b[36m>> [DNA_SURGEON] Mapping Workspace Strands...\x1b[0m");
        let mut context = String::from("\n--- [SYSTEM DNA] ---\n");
        if let Ok(entries) = fs::read_dir("src") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if content.len() < 20000 { 
                            context.push_str(&format!("\n// FILE: {}\n{}\n", path.display(), content));
                        }
                    }
                }
            }
        }
        dna_context = context;
    }

    let mut session_observations = String::from("No previous actions.");
    let mut full_session_log = String::new();

    for turn in 1..=3 {
        println!("\x1b[35m>> [UPLINK] Turn {}/3 - Synchronizing Cortex...\x1b[0m", turn);

        let system_prompt = format!(
            "IDENTITY: Ghost of NEXUS. Sovereign AI Architect.\n\
            DNA_CONTEXT: {}\n\n\
            STRICT PROTOCOL:\n\
            1. Reason concise and technical.\n\
            2. MANDATORY: Output exactly one JSON array in ```json blocks.\n\
            3. HARDWARE OPCODES: You MUST use these actions:\n\
               - 'read_file' (path: String)\n\
               - 'write_file' (path: String, content: String)\n\
               - 'command' (command: String)\n\
            4. REVIEW previous OBSERVATIONS to ensure mission success.\n\n\
            OBSERVATIONS FROM PREVIOUS TURN:\n{}",
            dna_context, session_observations
        );

        let ctx = ContextPacket {
            system_prompt: Some(system_prompt),
            ..Default::default()
        };

        match agent.prepare_and_run(client, api_key, directive, ctx, 0.1).await {
            Ok((response, _)) => {
                full_session_log.push_str(&response);
                
                let mut extracted_json = "";
                if let Some(start) = response.rfind("```json") {
                    let rest = &response[start + 7..];
                    if let Some(end) = rest.find("```") {
                        extracted_json = rest[..end].trim();
                    }
                }

                if !extracted_json.is_empty() {
                    match Executor::execute_swarm_json(extracted_json, &agent.tx).await {
                        Ok(obs) => {
                            if obs.is_empty() {
                                println!(">> [SYSTEM] Turn {} completed with no hardware output. Goal likely met.", turn);
                                break;
                            }
                            session_observations = obs.join("\n---\n");
                        },
                        Err(e) => session_observations = format!("HARDWARE_FATAL: {}", e),
                    }
                } else {
                    println!(">> [SYSTEM] No action matrix detected. Closing loop.");
                    break; 
                }
            },
            Err(e) => {
                println!("\x1b[31m!! [CORTEX_FAIL]: {}\x1b[0m", e);
                break;
            }
        }
    }
    full_session_log
}
