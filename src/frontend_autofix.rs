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


use crate::neural_agent::{ContextPacket, MicroAgent};
use std::process::Command;

pub async fn fix_ui(
    force: bool,
    agent: &MicroAgent,
    client: &reqwest::Client,
    api_key: &str,
) {
    println!(">> [AUTOFIX] Correlating UI faults with Gemma-3...");

    let plan_prompt = format!(
        "Fix the Svelte frontend. Force mode: {}. Provide a 3-step repair plan.",
        force
    );

    let ctx = ContextPacket::default();
    
    if let Ok((plan, _)) = agent.prepare_and_run(client, api_key, &plan_prompt, ctx, 0.5).await {
        println!("\n\x1b[36m[REPAIR PLAN]\x1b[0m\n{}", plan);
    }

    // AUDITOR FIX: Explicit Status Checking (No more blind execution)
    if force {
        println!("   • Force: Purging artifacts...");
        let _ = Command::new("rm").args(["-rf", "ui/node_modules", "ui/dist"]).status();
    }
    
    println!("   • Ingesting dependencies...");
    let install_status = Command::new("npm").current_dir("ui").arg("install").status();
    
    if let Ok(s) = install_status {
        if s.success() {
            println!("   • Integrating UI strands...");
            let build_status = Command::new("npm").current_dir("ui").args(["run", "build"]).status();
            if let Ok(bs) = build_status {
                if bs.success() {
                    println!("\x1b[32m>> [SUCCESS] UI Zenith Reached.\x1b[0m");
                    return;
                }
            }
        }
    }

    eprintln!("\x1b[31m!! [CRITICAL] UI Repair failed. Manual triage required.\x1b[0m");
}
