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
    let mut dna_context = String::new();
    let low_dir = directive.to_lowercase();
    
    // Safety injection for audit/fix directives
    if low_dir.contains("audit") || low_dir.contains("code") || low_dir.contains("fix") {
        println!("\x1b[36m>> [DNA_SURGEON] Mapping Workspace Strands...\x1b[0m");
        if let Ok(entries) = fs::read_dir("src") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |e| e == "rs") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if content.len() < 20000 { 
                            dna_context.push_str(&format!("\n// FILE: {}\n{}\n", path.display(), content));
                        }
                    }
                }
            }
        }
    }

    let system_prompt = format!(
        "IDENTITY: NEXUS Ghost. Sovereign AI Architect.\n\
        DNA_CONTEXT: {}\n\n\
        STRICT PROTOCOL:\n\
        1. Reason regarding the architecture.\n\
        2. MANDATORY: Output exactly one JSON array inside ```json blocks at the end.\n\
        3. Use action: 'write_file' for all creation/mutations.\n\
        4. Use action: 'command' for shell execution.",
        dna_context
    );

    let ctx = ContextPacket {
        system_prompt: Some(system_prompt),
        ..Default::default()
    };

    println!("\x1b[35m>> [UPLINK] Querying Gemma-4 MoE Core...\x1b[0m");

    match agent.prepare_and_run(client, api_key, directive, ctx, 0.1).await {
        Ok((response, _)) => {
            if let Some(start) = response.find("```json") {
                let rest = &response[start + 7..];
                if let Some(end) = rest.find("```") {
                    let _ = Executor::execute_swarm_json(rest[..end].trim(), &agent.tx).await;
                }
            } else if response.contains("[") && response.contains("]") {
                 // Fallback for missing backticks
                 let _ = Executor::execute_swarm_json(&response, &agent.tx).await;
            }
            response
        },
        Err(e) => {
            println!("\x1b[31m!! [CORTEX_FAIL]: {}\x1b[0m", e);
            e.to_string()
        }
    }
}
