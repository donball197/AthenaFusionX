// src/janitor.rs
use std::sync::Arc;
use crate::server::AppState;
use crate::neural_agent::ContextPacket;
use tokio::time::{sleep, Duration};

pub struct Janitor;

impl Janitor {
    pub async fn start_background_maintenance(state: Arc<AppState>) {
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(300)).await; // Check every 5 minutes
                
                let vitals = state.monitor.get_vitals();
                
                // If system is idle, enter the Dream Cycle
                if vitals.cpu_load < 5.0 {
                    // FIX: Lock mutex for read
                    let memories = state.memory.lock().await.load_all();
                    
                    if memories.len() > 30 {
                        println!(">> [JANITOR] ENTERING DREAM CYCLE: Compressing {} memories...", memories.len());
                        
                        let mut raw_text = String::from("Summarize these technical achievements into a single archetype:\n");
                        for m in &memories {
                            raw_text.push_str(&format!("- {}\n", m.prompt));
                        }

                        let packet = ContextPacket::default();
                        if let Ok((summary, _)) = state.agent.prepare_and_run(
                            &state.client, 
                            &state.api_key, 
                            &raw_text, 
                            packet, 
                            0.2
                        ).await {
                            // FIX: Lock mutex for write/purge
                            state.memory.lock().await.purge_to_archive(summary);
                            println!(">> [JANITOR] DREAM CYCLE COMPLETE. Memories archived.");
                        }
                    }
                }
                
                // General Cleanup
                let _ = std::process::Command::new("cargo").args(["clean", "--target-dir", "target/release/incremental"]).status();
            }
        });
    }

    pub fn cleanup_build_artifacts() -> anyhow::Result<String> {
        let _ = std::process::Command::new("cargo").args(["clean", "-p", "nexus_singularity"]).status();
        Ok("Artifacts purged.".into())
    }
}
