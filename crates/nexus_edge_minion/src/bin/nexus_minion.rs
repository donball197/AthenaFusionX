//! Nexus Edge Minion Binary Runner - Multi-Step Autoregressive Stress Test
//! Orchestrates local inference execution against the USB safetensors substrate.

use nexus_edge_minion::api_gateway::{AgentRequest, ApiGateway};
use nexus_edge_minion::inference_engine::InferenceEngine;
use std::env;
use std::env::consts;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Expected invocation: nexus_minion --tier <light|heavy> <mission text>
    let mut tier = "light".to_string();
    let mut prompt = "Substrate readiness verified.".to_string();
    let mut prompt_set = false;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--tier" && i + 1 < args.len() {
            tier = args[i + 1].clone();
            i += 2;
        } else {
            prompt = args[i].clone();
            prompt_set = true;
            i += 1;
        }
    }
    let _ = prompt_set;

    println!(">> [NEXUS MINION] Initializing execution pipeline...");
    println!(">> [NEXUS MINION] Task Tier: \"{}\"", tier);
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
                println!(
                    ">> [STRESS TEST] Running multi-step autoregressive loop ({} tokens)...",
                    num_steps
                );

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
                println!(
                    ">> [STRESS TEST] Completed {} steps in {:.2}s ({:.2} tokens/sec)",
                    num_steps,
                    total_duration.as_secs_f64(),
                    num_steps as f64 / total_duration.as_secs_f64()
                );
            }
            Err(e) => {
                eprintln!(">> [LOCAL EDGE ERROR] Failed to initialize local engine: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!(">> [CLOUD ROUTE] NEXUS_CACHE_DIR not set. Dispatching to API Gateway...");

        let cascade_env = if tier == "heavy" {
            env::var("NEXUS_HEAVY_CASCADE").unwrap_or_else(|_| {
                env::var("NEXUS_HEAVY_MODEL").unwrap_or_else(|_| "gemini-2.5-pro".to_string())
            })
        } else {
            env::var("NEXUS_LIGHT_CASCADE").unwrap_or_else(|_| {
                env::var("NEXUS_LIGHT_MODEL").unwrap_or_else(|_| {
                    env::var("NEXUS_PLANNER_MODEL")
                        .unwrap_or_else(|_| "gemini-3.5-flash-lite".to_string())
                })
            })
        };

        let cascade: Vec<String> = cascade_env.split(',').map(|s| s.trim().to_string()).collect();

        let workspace_root =
            env::var("NEXUS_WORKSPACE_ROOT").map(std::path::PathBuf::from).unwrap_or_else(|_| {
                env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });

        println!(">> [GATEWAY] Workspace root for tools: {}", workspace_root.display());

        let gateway = ApiGateway::new();
        let request = AgentRequest { prompt: prompt.clone(), temperature: 0.2, max_tokens: 2048 };

        match gateway.dispatch_with_tools(request, &cascade, &workspace_root) {
            Ok(response) => {
                println!(">> [GATEWAY] Model used: {}", response.model_used);
                println!(">> [GATEWAY] Tokens used: {}", response.tokens_used);
                println!("---- RESULT ----");
                println!("{}", response.text);
                println!("----------------");
            }
            Err(e) => {
                eprintln!(">> [GATEWAY ERROR] {}", e);
                std::process::exit(1);
            }
        }
    }

    println!(">> [NEXUS MINION] Execution cycle complete.");
}
