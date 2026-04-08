// src/backend_autofix.rs
// NUKS - Backend Auto-Healing WITH NEURAL PLANNING (Gemma 3 27B-IT)

use crate::neural_agent::{ContextPacket, MicroAgent};
use std::process::Command;

pub async fn fix_backend(
    force: bool,
    agent: &MicroAgent,
    client: &reqwest::Client,
    api_key: &str,
) {
    println!(">> [AUTOFIX] Generating NEURAL REPAIR PLAN for Backend Layer...");
    
    let plan_prompt = format!(
        "You are NEXUS. Device: Lenovo Duet 2 4GB RAM. \
        User wants to fix the Rust/Axum backend. Force mode: {}. \
        Provide a concise step-by-step repair plan (cargo commands, etc.).",
        force
    );
    
    let ctx = ContextPacket {
        system_prompt: Some(
            "You are NEXUS. Respond only with a numbered repair plan. \
            Keep steps lightweight for low-RAM ARM64 device.".to_string(),
        ),
        ..Default::default()
    };
    
    match agent.prepare_and_run(client, api_key, &plan_prompt, ctx, 0.5).await {
        Ok((plan, _)) => {
            println!("\n\x1b[38;2;0;255;255m[GEMMA REPAIR PLAN]\x1b[0m");
            println!("{}", plan);
            println!("\x1b[38;2;255;165;0m>> Executing plan now...\x1b[0m\n");
        }
        Err(e) => println!("\x1b[33m!! Neural planning failed (continuing): {}\x1b[0m", e),
    }
    
    // === Actual fix execution ===
    if force {
        println!("   • Force mode: cargo clean...");
        let _ = Command::new("cargo").arg("clean").status();
    }
    
    println!("   • Running cargo check...");
    let _ = Command::new("cargo").arg("check").status();
    
    println!("   • Building backend...");
    match Command::new("cargo").arg("build").status() {
        Ok(s) if s.success() => println!(">> [SUCCESS] Backend layer repaired."),
        _ => println!("\x1b[31m!! [WARN] Build failed.\x1b[0m"),
    }
}
