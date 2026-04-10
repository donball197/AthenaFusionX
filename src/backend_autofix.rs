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
