/**
 * Copyright 2026 Donald Ball
 * Project Nexus Singularity
 */

// src/bin/ui_audit.rs - Native UI Build Interrogator
#[tokio::main]
async fn main() {
    println!(">> [INQUISITOR] Initiating UI Build Audit...");
    // ⚡ SUTURE: Await the async audit
    match nexus_singularity::inquisitor::Inquisitor::audit_ui().await {
        Ok(logs) => {
            println!("--- RAW VITE/SVELTE LOGS START ---");
            println!("{}", logs);
            println!("--- RAW VITE/SVELTE LOGS END ---");
        }
        Err(e) => {
            eprintln!("!! [FRACTURE] UI Audit failed: {}", e);
        }
    }
}
