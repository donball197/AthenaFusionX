// src/bin/alpha_scout.rs - ISOLATED ZIG-REFLEX TEST
use nexus_singularity::scraper_drone;
use nexus_singularity::synapse::NexusSynapse;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("\x1b[1;36m╔══════════════════════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m \x1b[1;37mALPHA-SCOUT: ZIG-ACCELERATED EXTRACTION DRONE\x1b[0m                                \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════════════════════╝\x1b[0m\n");

    let target_url = "https://example.com";
    
    // Ignite the Synapse (Ensure your localhost/nexus_synapse Podman container is running)
    println!(">> [SYNAPSE] Binding to Redis memory palace...");
    let synapse = NexusSynapse::ignite("redis://127.0.0.1/", "nexus_scrape_stream")
        .expect("!! [CORTEX_FAIL] Redis container unreachable. Is Podman running?");

    // Execute the Hybrid Extraction
    match scraper_drone::execute_mission(target_url, &synapse).await {
        Ok(_) => println!("\x1b[32m>> [ZENITH] Mission successful. Data streamed to Redis.\x1b[0m"),
        Err(e) => println!("\x1b[31m!! [FRACTURE] Mission failed: {}\x1b[0m", e),
    }

    Ok(())
}
