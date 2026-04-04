use crate::neural_agent::{ContextPacket, MicroAgent};
use std::env;
use tokio::sync::broadcast;
use reqwest::Client;

pub struct SyntaxSentinel;

pub async fn run_sovereign_audit(payload_path: &str) {
    let api_key = env::var("GEMINI_API_KEY").unwrap_or_default();
    let client = Client::new();
    let (tx, _) = broadcast::channel(16);
    let agent = MicroAgent::init(tx).expect("Agent Init Failure");

    let payload = std::fs::read_to_string(payload_path).unwrap_or_default();

    let ctx = ContextPacket {
        system_prompt: Some(
            "You are the NEXUS SOVEREIGN AUDITOR. 
             STRICT PROTOCOL:
             1. Categorize issues: CRITICAL, HIGH, MEDIUM, LOW.
             2. Assign CVSS v3.1 scores to all security vulnerabilities.
             3. Evaluate EXPLOITABILITY (How easy is it to trigger this bug?).
             4. Provide a concrete REMEDIATION PLAN for each finding.".into()
        ),
        ..Default::default()
    };

    println!(">> [AUDITOR] Initiating High-Fidelity Security Sweep...");
    
    match agent.prepare_and_run(&client, &api_key, &payload, ctx, 0.2).await {
        Ok((_report, _)) => {
            println!("\n>> [ZENITH AUDIT COMPLETE]");
        }
        Err(e) => println!("!! [AUDIT_FAULT] {}", e),
    }
}

pub fn run_integrity_check() -> Result<(), String> {
    if std::path::Path::new("src/main.rs").exists() { Ok(()) } 
    else { Err("VITAL_CORE_MISSING".into()) }
}
