//! Nexus Edge Minion Binary Runner - Multi-Step Autoregressive Stress Test
//! Orchestrates local inference execution against the USB safetensors substrate.

use std::env::consts;
use std::env;
use std::time::Instant;
use nexus_edge_minion::inference_engine::InferenceEngine;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let prompt = if args.len() > 1 {
        args[1].clone()
    } else {
        "Substrate readiness verified.".to_string()
    };

    println!(">> [NEXUS MINION] Initializing execution pipeline...");
    println!(">> [NEXUS MINION] Task Prompt: \"{}\"", prompt);
    println!(">> [HOST ARCH] Target Architecture: {}-{}", consts::ARCH, consts::OS);

    if let Ok(cache_dir) = env::var("NEXUS_CACHE_DIR") {
        println!(">> [LOCAL EDGE] Detected NEXUS_CACHE_DIR: {}", cache_dir);
        println!(">> [LOCAL EDGE] Engaging local inference helper engine...");

        match InferenceEngine::new(&cache_dir) {
            Ok(mut engine) => {
                println!(">> [LOCAL EDGE] Safetensors weights mapped successfully.");
                let mut hidden_state = vec![0.0f32; 1536];
                
                let num_steps = 5;
                println!(">> [STRESS TEST] Running multi-step autoregressive loop ({} tokens)...", num_steps);
                
                let start_time = Instant::now();
                for pos in 0..num_steps {
                    let token_id = (100 + pos) as u32;
                    let step_start = Instant::now();
                    
                    match engine.step(token_id, pos, &mut hidden_state) {
                        Ok(_) => {
                            let duration = step_start.elapsed();
                            println!(
                                "  -> [STEP {}/{}]: Token ID {} processed in {:.2}ms (KV-cache active)",
                                pos + 1, num_steps, token_id, duration.as_secs_f64() * 1000.0
                            );
                        }
                        Err(e) => {
                            eprintln!("  -> [ERROR] Step {} failed: {}", pos + 1, e);
                            break;
                        }
                    }
                }
                
                let total_duration = start_time.elapsed();
                println!(">> [STRESS TEST] Completed {} steps in {:.2}s ({:.2} tokens/sec)", 
                    num_steps, 
                    total_duration.as_secs_f64(), 
                    num_steps as f64 / total_duration.as_secs_f64()
                );
            }
            Err(e) => {
                eprintln!(">> [LOCAL EDGE ERROR] Failed to initialize local engine: {}", e);
            }
        }
    } else {
        println!(">> [CLOUD ROUTE] NEXUS_CACHE_DIR not set. Handing off to API Gateway powerhouse...");
    }

    println!(">> [NEXUS MINION] Execution cycle complete.");
}
