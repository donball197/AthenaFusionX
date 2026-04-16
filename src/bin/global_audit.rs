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


// src/bin/global_audit.rs - Native Hierarchical Map-Reduce Auditor
use anyhow::Result;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;
use nexus_singularity::neural_agent::{MicroAgent, ContextPacket};

struct Domain {
    name: &'static str,
    paths: Vec<&'static str>,
}

fn read_domain_files(paths: &[&str]) -> String {
    let mut payload = String::new();
    for path_str in paths {
        let path = Path::new(path_str);
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                payload.push_str(&format!("\n--- FILE: {} ---\n{}\n", path_str, content));
            }
        } else if path.is_dir() {
            // Basic flat directory read for UI/Kernel
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path.is_file() {
                        if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                            if matches!(ext, "rs" | "svelte" | "js" | "ts" | "zig") {
                                if let Ok(content) = fs::read_to_string(&file_path) {
                                    payload.push_str(&format!("\n--- FILE: {} ---\n{}\n", file_path.display(), content));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    payload
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();
    let (tx, _) = broadcast::channel(16);
    let agent = Arc::new(MicroAgent::init(tx)?);

    println!("\x1b[35m>> [NEXUS] INITIATING NATIVE HIERARCHICAL AUDIT (RATE-LIMITED)\x1b[0m");

    let domains = vec![
        Domain { name: "UI", paths: vec!["ui/src"] },
        Domain { name: "KERNEL", paths: vec!["src/kernel", "src/reflex"] },
        Domain { name: "CORTEX", paths: vec!["src/neural_agent.rs", "src/task_manager.rs", "src/orchestrator.rs"] },
        Domain { name: "INFRASTRUCTURE", paths: vec!["src/server.rs", "src/executor.rs", "src/lazarus_vault.rs"] },
    ];

    let mut grand_synthesis_payload = String::from("=== GLOBAL SYSTEM ARCHITECTURE ===\n\n");

    // 1. THE MAP PHASE
    for domain in domains {
        println!("\n\x1b[36m[*] Mapping Domain: {}\x1b[0m", domain.name);
        let code_payload = read_domain_files(&domain.paths);
        
        if code_payload.is_empty() {
            println!("    -> No readable code found. Skipping.");
            continue;
        }

        println!("    -> Compiling {} sector via Gemma API...", domain.name);
        
        let prompt = format!(
            "Analyze the following codebase section for the {} domain.\n\
            Provide a dense, highly technical summary of its architectural role, key structs/functions, and any logic bottlenecks.\n\n\
            CODE:\n{}", domain.name, code_payload
        );

        let ctx = ContextPacket {
            system_prompt: Some("You are a Senior Systems Architect. Output a concise technical summary. No pleasantries.".into()),
            ..Default::default()
        };

        match agent.prepare_and_run(&client, &api_key, &prompt, ctx, 0.3, None).await {
            Ok((summary, _)) => {
                grand_synthesis_payload.push_str(&format!("--- {} DOMAIN SUMMARY ---\n{}\n\n", domain.name, summary));
                println!("    -> Domain summarized successfully.");
            }
            Err(e) => println!("\x1b[31m    -> API Failure on {}: {}\x1b[0m", domain.name, e),
        }

        println!("\x1b[90m    [+] Cooling down API token window (15s)...\x1b[0m");
        tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
    }

    // 2. THE REDUCE PHASE
    println!("\n\x1b[35m[*] Initiating Grand Synthesis...\x1b[0m");
    
    let synth_prompt = format!(
        "You are the Lead Architect. Review these domain summaries and provide a unified 'Grand Synthesis' of the entire system architecture. \
        Identify how the domains interact and point out any cross-domain fracture points.\n\n{}", 
        grand_synthesis_payload
    );

    let synth_ctx = ContextPacket {
        system_prompt: Some("You are the NEXUS Sovereign AI. Output a highly structured, systemic analysis.".into()),
        ..Default::default()
    };

    match agent.prepare_and_run(&client, &api_key, &synth_prompt, synth_ctx, 0.7, None).await {
        Ok((final_report, _)) => {
            println!("\n\x1b[32m=== [GLOBAL SYNTHESIS REPORT] ===\x1b[0m\n{}", final_report);
            std::fs::write(".nexus_global_synthesis.md", final_report)?;
            println!("\n\x1b[32m>> [GLOBAL AUDIT COMPLETE] Report saved to .nexus_global_synthesis.md\x1b[0m");
        }
        Err(e) => println!("\x1b[31m>> Synthesis API Failure: {}\x1b[0m", e),
    }

    Ok(())
}
