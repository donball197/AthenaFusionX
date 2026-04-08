use crate::neural_agent::{ContextPacket, MicroAgent};
use crate::executor::Executor;
use crate::memory::MemoryPalace;
use crate::polyglot;
use crate::frontend_inspector::UIInspector;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;
use walkdir::WalkDir; // SUTURE: Restored missing import

pub async fn send_task(
    agent: Arc<MicroAgent>, 
    client: &reqwest::Client, 
    api_key: &str, 
    directive: &str,
    memory: Arc<Mutex<MemoryPalace>>
) -> String {
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
                    .collect::<Vec<_>>()
                    .join("\n---\n");
            }
        }
    }

    // 2. PROJECT MAP & MANIFEST
    if let Ok(cargo) = fs::read_to_string("Cargo.toml") {
        dna_context.push_str(&format!("--- [DEPENDENCY MANIFEST] ---\n{}\n", cargo));
    }
    dna_context.push_str(&format!("\n--- [PROJECT TOPOLOGY] ---\n{}\n", polyglot::generate_project_tree()));

    // 3. FULL-STACK DNA SLURPER
    if low_dir.contains("ui") || low_dir.contains("frontend") || low_dir.contains("svelte") {
        let ui_targets = vec!["ui/src/routes/+page.svelte", "ui/src/lib/components/Terminal.svelte"];
        for path in ui_targets {
            if let Ok(analysis) = UIInspector::scan_svelte_logic(path) {
                dna_context.push_str(&format!("\n--- [FRONTEND DNA: {}] ---\n{}\n", path, analysis));
            }
        }
    }

    if low_dir.contains("audit") || low_dir.contains("code") || low_dir.contains("fix") || low_dir.contains("refactor") {
        for dir in ["src", "ui/src"] {
            for entry in WalkDir::new(dir).max_depth(3).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() && entry.path().extension().map_or(false, |e| e == "rs" || e == "svelte") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if content.len() > 10000 {
                            let head = &content[..5000];
                            let tail = &content[content.len()-2000..];
                            dna_context.push_str(&format!("\n// FILE: {} [SKELETON]\n{}\n// [...]\n{}\n", entry.path().display(), head, tail));
                        } else {
                            dna_context.push_str(&format!("\n// FILE: {}\n{}\n", entry.path().display(), content));
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
            1. Reason concisely. Use Svelte 5 Runes.\n\
            2. MANDATORY: Output exactly one JSON array in ```json blocks.\n\
            3. OPCODES: 'read_file', 'write_file', 'command'.\n\n\
            OBSERVATIONS:\n{}",
            wisdom, dna_context, session_observations
        );

        let ctx = ContextPacket { system_prompt: Some(system_prompt), ..Default::default() };

        match agent.prepare_and_run(client, api_key, directive, ctx, 0.1).await {
            Ok((response, _)) => {
                full_session_log.push_str(&response);
                let mut extracted_json = "";
                if let Some(start) = response.rfind("```json") {
                    let rest = &response[start + 7..];
                    if let Some(end) = rest.find("```") { extracted_json = rest[..end].trim(); }
                }

                if !extracted_json.is_empty() {
                    match Executor::execute_swarm_json(extracted_json, &agent.tx).await {
                        Ok(obs) => {
                            let obs_text = obs.join("\n---\n");
                            if obs_text.contains("SECURITY_VIOLATION") { return obs_text; }
                            if turn < 3 { session_observations = obs_text; }
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
