use anyhow::Result;
use nexus_singularity::neural_agent::{ContextPacket, MicroAgent};
use std::fs;
use std::sync::Arc;
use tokio::sync::broadcast;
use walkdir::WalkDir;
use std::path::PathBuf;

struct Domain {
    name: &'static str,
    paths: Vec<&'static str>,
}

async fn analyze_domain_hierarchical(
    domain: &Domain,
    agent: &Arc<MicroAgent>,
    client: &reqwest::Client,
    api_key: &str,
) -> Result<String> {
    let mut file_stack: Vec<PathBuf> = Vec::new();
    let mut current_batch = String::new();
    let mut batch_summaries = Vec::new();
    let max_batch_chars = 30_000;

    println!("\x1b[36m[*] Hierarchical Scan: {}\x1b[0m", domain.name);
    for path in &domain.paths {
        for entry in WalkDir::new(path).into_iter().flatten() {
            let p = entry.path();
            if p.is_file() {
                // ⚡ FIX: Explicitly typed closure parameter for extension matching
                if let Some(ext) = p.extension().and_then(|s: &std::ffi::OsStr| s.to_str()) {
                    if matches!(ext, "rs" | "svelte" | "js" | "ts" | "zig" | "json") {
                        if p.to_string_lossy().contains("node_modules") || p.to_string_lossy().contains("-lock.json") {
                            continue;
                        }
                        file_stack.push(p.to_owned());
                    }
                }
            }
        }
    }

    for file_path in file_stack {
        // ⚡ FIX: Specify the generic path type for read_to_string
        if let Ok(content) = fs::read_to_string(&file_path) {
            let entry_text = format!("\n--- FILE: {} ---\n{}\n", file_path.display(), content);
            if current_batch.len() + entry_text.len() > max_batch_chars {
                let summary = request_batch_summary(agent, client, api_key, &current_batch, domain.name).await?;
                batch_summaries.push(summary);
                current_batch.clear();
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
            current_batch.push_str(&entry_text);
        }
    }

    if !current_batch.is_empty() {
        let summary = request_batch_summary(agent, client, api_key, &current_batch, domain.name).await?;
        batch_summaries.push(summary);
    }

    println!("    -> Reducing {} batch summaries into Domain Report...", batch_summaries.len());
    let synthesis_prompt = format!(
        "Synthesize these sector summaries into a single, dense architectural report for the {} domain.\n\n{}",
        domain.name, batch_summaries.join("\n\n---\n\n")
    );
    let (domain_report, _) = agent.prepare_and_run(client, api_key, &synthesis_prompt, ContextPacket::default(), 0.4, None).await?;
    Ok(domain_report)
}

async fn request_batch_summary(
    agent: &Arc<MicroAgent>,
    client: &reqwest::Client,
    api_key: &str,
    content: &str,
    domain_name: &str,
) -> Result<String> {
    println!("    -> Mapping batch ({} bytes)...", content.len());
    let prompt = format!("Analyze this code batch from the {} domain. Provide a dense technical summary.\n\n{}", domain_name, content);
    let ctx = ContextPacket {
        system_prompt: Some("You are a Lead Architect. Provide a concise technical summary.".into()),
        ..Default::default()
    };
    let (summary, _) = agent.prepare_and_run(client, api_key, &prompt, ctx, 0.3, None).await?;
    Ok(summary)
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();
    let (tx, _) = broadcast::channel(16);
    let agent = Arc::new(MicroAgent::init(tx)?);
    println!("\x1b[35m>> [NEXUS] INITIATING HIERARCHICAL RECURSIVE AUDIT\x1b[0m");

    let domains = vec![
        Domain { name: "UI", paths: vec!["ui/src"] },
        Domain { name: "KERNEL", paths: vec!["src/kernel"] },
        Domain { name: "CORTEX", paths: vec!["src/neural_agent.rs", "src/task_manager.rs", "src/orchestrator.rs"] },
        Domain { name: "INFRASTRUCTURE", paths: vec!["src/server.rs", "src/executor.rs", "src/lazarus_vault.rs"] },
    ];

    let mut domain_reports = Vec::new();
    for domain in domains {
        let report = analyze_domain_hierarchical(&domain, &agent, &client, &api_key).await?;
        domain_reports.push(format!("### {} DOMAIN REPORT\n{}", domain.name, report));
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }

    let synth_prompt = format!("Perform a Grand Synthesis of the system architecture. Identify systemic vulnerabilities.\n\n{}", domain_reports.join("\n\n"));
    match agent.prepare_and_run(&client, &api_key, &synth_prompt, ContextPacket::default(), 0.7, None).await {
        Ok((final_report, _)) => {
            std::fs::write(".nexus_global_synthesis.md", final_report)?;
            println!("\x1b[32m>> [GLOBAL AUDIT COMPLETE] Report saved to .nexus_global_synthesis.md\x1b[0m");
        }
        Err(e) => println!("\x1b[31m>> Synthesis API Failure: {}\x1b[0m", e),
    }
    Ok(())
}
