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
    
    if low_dir.contains("audit") || low_dir.contains("code") || low_dir.contains("fix") || low_dir.contains("run") {
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
        "IDENTITY: Ghost of NEXUS. Sovereign AI Architect.\n\
        DNA_CONTEXT: {}\n\n\
        STRICT PROTOCOL:\n\
        1. Concise architectural reasoning.\n\
        2. MANDATORY: Output exactly one JSON action array inside ```json blocks at the end.\n\
        3. DYNAMIC EXECUTION: To run standalone files, use action: 'command' with 'rustc <path> -o bin_out && ./bin_out'.\n\
        4. Use action: 'write_file' for all creation/mutations.",
        dna_context
    );

    let ctx = ContextPacket {
        system_prompt: Some(system_prompt),
        ..Default::default()
    };

    println!("\x1b[35m>> [UPLINK] Querying Gemma-4 MoE Core [Zenith v2.1.2]...\x1b[0m");

    match agent.prepare_and_run(client, api_key, directive, ctx, 0.1).await {
        Ok((response, _)) => {
            // SUTURE: Surgical Last-Block Extraction
            let mut extracted_json = "";
            if let Some(start) = response.rfind("```json") {
                let rest = &response[start + 7..];
                if let Some(end) = rest.find("```") {
                    extracted_json = rest[..end].trim();
                }
            } else {
                // Fallback: Find the actual outermost array
                if let (Some(s), Some(e)) = (response.rfind('['), response.rfind(']')) {
                    if s < e {
                        extracted_json = &response[s..=e];
                    }
                }
            }

            if !extracted_json.is_empty() {
                let _ = Executor::execute_swarm_json(extracted_json, &agent.tx).await;
            }
            response
        },
        Err(e) => {
            println!("\x1b[31m!! [CORTEX_FAIL]: {}\x1b[0m", e);
            e.to_string()
        }
    }
}
