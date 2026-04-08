use crate::neural_agent::{ContextPacket, MicroAgent};
use std::env;
use tokio::sync::broadcast;
use reqwest::Client;
use anyhow::{Result, Context};

pub struct SyntaxSentinel;

pub async fn run_sovereign_audit(payload_path: &str) -> Result<()> {
    let api_key = env::var("GEMINI_API_KEY").context("GEMINI_API_KEY missing from environment")?;
    let client = Client::new();
    let (tx, _) = broadcast::channel(16);
    let agent = MicroAgent::init(tx).context("Failed to initialize Auditor agent")?;

    let payload = std::fs::read_to_string(payload_path)
        .context(format!("Failed to read audit target: {}", payload_path))?;

    let ctx = ContextPacket {
        system_prompt: Some("You are the NEXUS SOVEREIGN AUDITOR. Identify CRITICAL vulnerabilities.".into()),
        ..Default::default()
    };

    println!(">> [AUDITOR] Initiating Security Sweep...");
    
    match agent.prepare_and_run(&client, &api_key, &payload, ctx, 0.2).await {
        Ok((report, _)) => {
            println!("\n>> [ZENITH AUDIT COMPLETE]");
            println!("{}", report);
            Ok(())
        }
        Err(e) => Err(e.context("Security audit synthesis failed")),
    }
}

pub fn run_integrity_check() -> Result<()> {
    if std::path::Path::new("src/main.rs").exists() { Ok(()) } 
    else { Err(anyhow::anyhow!("VITAL_CORE_MISSING: src/main.rs is not found")) }
}
