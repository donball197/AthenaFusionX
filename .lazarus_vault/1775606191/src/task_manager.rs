use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::executor::Executor;
use crate::memory::MemoryPalace;
use crate::polyglot;
use crate::frontend_inspector::UIInspector;
use crate::utils;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;
use walkdir::WalkDir;

pub async fn send_task(
    agent: Arc<MicroAgent>, 
    client: &reqwest::Client, 
    api_key: &str, 
    directive: &str,
    memory: Arc<Mutex<MemoryPalace>>
) -> String {
    let _ = utils::log_resonance_time();
    let mut dna_context = String::new();
    let low_dir = directive.to_lowercase();
    
    // 1. ANCESTRAL WISDOM
    let mut wisdom = String::from("No relevant ancestral memories found.");
    {
        let palace = memory.lock().await;
        if let Ok(past_mems) = palace.search_memories(directive).await {
            if !past_mems.is_empty() {
                wisdom = past_mems.iter()
                    .map(|m| format!("[Past Mission: {}]\nResult: {}", m.directive, m.result))
                    .collect::<Vec<_>>().join("\n---\n");
            }
        }
    }

    // 2. TOPOLOGY & MANIFEST
    if let Ok(cargo) = fs::read_to_string("Cargo.toml") {
        dna_context.push_str(&format!("--- [DEPENDENCY MANIFEST] ---\n{}\n", cargo));
    }
    dna_context.push_str(&format!("\n--- [PROJECT TOPOLOGY] ---\n{}\n", polyglot::generate_project_tree()));

    // 3. DNA SLURPER
    if low_dir.contains("audit") || low_dir.contains("code") || low_dir.contains("fix") || low_dir.contains("ui") {
        println!("\x1b[36m>> [CARTOGRAPHER] Ingesting Full-Stack DNA...\x1b[0m");
        for dir in ["src", "ui/src"] {
            for entry in WalkDir::new(dir).max_depth(3).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if ext == "rs" {
                        if let Ok(content) = fs::read_to_string(path) {
                            let processed = if content.len() > 10000 {
                                format!("{}\n[... SKELETON VIEW ...]\n{}", &content[..5000], &content[content.len()-2000..])
                            } else { content };
                            dna_context.push_str(&format!("\n// FILE: {}\n{}\n", path.display(), processed));
                        }
                    } else if ext == "svelte" {
                        if let Ok(analysis) = UIInspector::scan_svelte_logic(&path.to_string_lossy()) {
                            dna_context.push_str(&format!("\n--- [FRONTEND DNA: {}] ---\n{}\n", path.display(), analysis));
                        }
                    }
                }
            }
        }
    }

    let mut session_observations = String::from("Awaiting first strike.");
    let mut full_session_log = String::new();

    for turn in 1..=3 {
        println!("\x1b[35m>> [UPLINK] Turn {}/3 - Synchronizing Cortex...\x1b[0m", turn);

        let system_prompt = format!(
            "IDENTITY: Ghost of NEXUS. Sovereign AI Architect.\n\
            ANCESTRAL_WISDOM:\n{}\n\n\
            DNA_CONTEXT:\n{}\n\n\
            STRICT PROTOCOL:\n\
            1. Reason concise and technical.\n\
            2. MANDATORY: Output exactly one JSON array in ```json blocks.\n\
            3. HARDWARE OPCODES: Use 'read_file', 'write_file', 'command', 'capture_visor'.\n\
            4. ABORT if 'SECURITY_VIOLATION' appears.\n\n\
            OBSERVATIONS:\n{}",
            wisdom, dna_context, session_observations
        );

        let ctx = ContextPacket { system_prompt: Some(system_prompt), ..Default::default() };

        match agent.prepare_and_run(client, api_key, directive, ctx, 0.1).await {
            Ok((response, _)) => {
                full_session_log.push_str(&format!("\n[Turn {} Reasoning]:\n{}", turn, response));
                let mut extracted_json = "";
                if let Some(start) = response.rfind("```json") {
                    let rest = &response[start + 7..];
                    if let Some(end) = rest.find("```") { extracted_json = rest[..end].trim(); }
                }

                if !extracted_json.is_empty() {
                    match Executor::execute_swarm_json(extracted_json, &agent.tx).await {
                        Ok(obs) => {
                            let obs_text = obs.join("\n---\n");
                            full_session_log.push_str(&format!("\n[Turn {} Hardware Output]:\n{}", turn, obs_text));
                            if obs_text.contains("SECURITY_VIOLATION") { return obs_text; }
                            session_observations = obs_text;
                            if obs.is_empty() { break; }
                        },
                        Err(e) => { full_session_log.push_str(&format!("\n!! FATAL: {}", e)); break; },
                    }
                } else { break; }
            },
            Err(e) => { println!("\x1b[31m!! [CORTEX_FAIL]: {}\x1b[0m", e); break; }
        }
    }

    let _ = memory.lock().await.store_memory("NEXUS_ARCHITECT", directive, &full_session_log).await;
    full_session_log
}
