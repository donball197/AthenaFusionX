use std::sync::Arc;
use tokio::sync::Mutex;
use crate::memory::MemoryPalace;
use crate::neural_agent::{MicroAgent, ContextPacket};
use crate::factory::AgentFactory;

pub async fn start_daydream(memory: Arc<Mutex<MemoryPalace>>, agent: Arc<MicroAgent>, api_key: String) {
    let client = reqwest::Client::new();
    let dream_model = AgentFactory::select_model_for_mission("dream");

    loop {
        // Sleep for 15 minutes between synthesis cycles
        tokio::time::sleep(std::time::Duration::from_secs(900)).await; 
        
        println!("\x1b[34m>> [ARCHIVIST] Identifying patterns for architectural optimization...\x1b[0m");
        
        let strands = {
            let palace = memory.lock().await;
            palace.fetch_random_strands(15).await.unwrap_or_default()
        };

        if strands.len() >= 5 {
            let mut context = String::new();
            for s in strands {
                context.push_str(&format!("Dir: {} | Res: {}\n", s.directive, s.result));
            }

            let prompt = format!("Summarize these logs into 3 high-density system insights:\n{}", context);
            
            // ⚡ FORCE THE 26B MoE FOR SPEED
            let ctx = ContextPacket { 
                system_prompt: Some(format!("You are the NEXUS Archivist. Model: {}", dream_model)),
                task_complexity: 1, 
                ..Default::default() 
            };

            if let Ok((insight, _)) = agent.prepare_and_run(&client, &api_key, &prompt, ctx, 0.8, None).await {
                let palace = memory.lock().await;
                if palace.store_insight(&insight).await.is_ok() {
                    println!("\x1b[32m>> [ARCHIVIST] Insights successfully enshrined to substrate.\x1b[0m");
                }
            }
        }
    }
}
