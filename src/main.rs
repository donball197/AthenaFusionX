mod hiram_core; mod neural_agent; mod refactor; mod auditor; mod broker; mod janitor;
use tokio::time::{sleep, Duration};
use std::{env, fs, process::Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let api_key = env::var("GEMINI_API_KEY")?;
    let node_id = env::var("NODE_ID").unwrap_or_else(|_| "UNKNOWN".to_string());
    let (client, agent) = (reqwest::Client::new(), neural_agent::MicroAgent::init()?);
    let refactor_engine = refactor::RefactorEngine::new();

    println!(">> [FORGE] NODE [{}] ONLINE. INDEPENDENT MODE.", node_id);

    loop {
        // Sync state with the global project
        let _ = Command::new("git").args(&["pull", "origin", "main"]).output();

        // 1. Check for Missions first (User Requests)
        if let Some(mission) = broker::get_unclaimed_mission(&node_id) {
            println!(">> [{}] CLAIMED MISSION: {:?}", node_id, mission.path);
            
            let prompt = format!("OBJECTIVE: {}. Generate code and return ONLY as CODE:", mission.objective);
            if let Ok(resp) = agent.evolve_kernel(&client, &api_key, &prompt, None).await {
                if resp.contains("CODE:") {
                    let code = resp.split("CODE:").collect::<Vec<&str>>()[1].trim();
                    let target = format!("src/gen_{}.rs", node_id.to_lowercase());
                    
                    if refactor_engine.apply_patch(&target, code).is_ok() {
                        broker::resolve_mission(mission);
                        sync_to_cloud(&node_id, &format!("Accomplished: {}", target));
                    }
                }
            }
        } else {
            // 2. Autonomous Evolution (If no missions)
            let target = janitor::find_target_file();
            let target_str = target.to_str().unwrap_or("src/hiram_core.rs");
            let current_code = fs::read_to_string(target_str)?;
            
            if let Ok(resp) = agent.evolve_kernel(&client, &api_key, &current_code, None).await {
                if resp.contains("CODE:") {
                    let new_code = resp.split("CODE:").collect::<Vec<&str>>()[1].trim();
                    if refactor_engine.apply_patch(target_str, new_code).is_ok() {
                        sync_to_cloud(&node_id, &format!("Optimized: {}", target_str));
                    }
                }
            }
        }
        sleep(Duration::from_secs(30)).await;
    }
}

fn sync_to_cloud(node: &str, msg: &str) {
    let _ = Command::new("git").args(&["add", "."]).output();
    let _ = Command::new("git").args(&["commit", "-m", &format!("[{}] {}", node, msg)]).output();
    let _ = Command::new("git").args(&["push", "origin", "main"]).output();
}
