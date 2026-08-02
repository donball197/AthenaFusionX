use futures_util::StreamExt;
use reqwest::Client;
use std::io::{self, Write};
use std::process::Command;
use tokio_tungstenite::connect_async;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    println!("\x1b[36m==================================================\x1b[0m");
    println!("\x1b[1;36m>> NEXUS SINGULARITY v4.0 — THIN CLIENT ONLINE\x1b[0m");
    println!("\x1b[36m==================================================\x1b[0m");

    let token = std::fs::read_to_string(".nexus_ui_token")
        .unwrap_or_else(|_| "".to_string())
        .trim()
        .to_string();

    let ws_url = "ws://127.0.0.1:3000/ws/telemetry";

    let ws_stream = match connect_async(ws_url).await {
        Ok((stream, _)) => Some(stream),
        Err(_) => {
            println!("\x1b[31m>> [ERROR] Could not connect to Daemon. Is it running?\x1b[0m");
            None
        }
    };

    if let Some(mut stream) = ws_stream {
        tokio::spawn(async move {
            let mut last_type = String::new();
            while let Some(Ok(msg)) = stream.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        let msg_type = parsed["type"].as_str().unwrap_or("LOG");
                        let payload = parsed["payload"].as_str().unwrap_or("");
                        match msg_type {
                            "NEURAL_THOUGHT" | "VITALS" | "HEARTBEAT" => {}
                            "NEURAL_STREAM" => {
                                if last_type != "NEURAL_STREAM" {
                                    print!(
                                        "\x1b[0m\n\x1b[1;36m[CORTEX EXECUTING]\x1b[0m\n\x1b[36m"
                                    );
                                }
                                print!("{}", payload);
                                let _ = io::stdout().flush();
                                last_type = "NEURAL_STREAM".to_string();
                            }
                            "TASK_STATUS" => {
                                println!("\x1b[0m\n\x1b[1;33m[SYS]\x1b[0m {}", payload);
                                last_type = "TASK_STATUS".to_string();
                            }
                            "LOG_EVENT" => {
                                println!("\x1b[0m\n\x1b[1;90m[EVENT]\x1b[0m {}", payload);
                                last_type = "LOG_EVENT".to_string();
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    }

    println!("\x1b[32m>> Neural Uplink Established. Substrate Ready.\x1b[0m");
    let client = Client::new();
    let stdin = io::stdin();

    loop {
        print!("\n\x1b[1;32mnexus>\x1b[0m (Type prompt, then 'EOF' on a new line to submit)\n> ");
        io::stdout().flush()?;

        let mut input = String::new();
        loop {
            let mut line = String::new();
            let bytes_read = stdin.read_line(&mut line)?;
            if bytes_read == 0 || line.trim() == "EOF" {
                break;
            }
            input.push_str(&line);
        }

        let directive = input.trim();
        if directive.is_empty() {
            continue;
        }
        if directive == "/exit" || directive == "/quit" {
            break;
        }

        if directive.starts_with("/mutate ") {
            let target_func = directive.strip_prefix("/mutate ").unwrap().trim();
            println!(
                "\x1b[33m>> [AST] Locating '{}' in repository_analysis/symbol_graph.json...\x1b[0m",
                target_func
            );

            // 1. Run the Impact Engine silently
            println!("\x1b[35m>> [IMPACT_ENGINE] Calculating Blast Radius...\x1b[0m");
            let _ = Command::new("cargo")
                .args(["run", "--bin", "impact_engine", "--", target_func])
                .output();

            let mut blast_radius_info = String::new();
            if let Ok(impact_data) =
                std::fs::read_to_string("repository_analysis/impact/latest_report.json")
            {
                if let Ok(impact) = serde_json::from_str::<serde_json::Value>(&impact_data) {
                    if let Some(nodes) = impact["affected_nodes"].as_array() {
                        let names: Vec<&str> = nodes.iter().filter_map(|n| n.as_str()).collect();
                        blast_radius_info = format!(
                            "WARNING: This mutation has a blast radius of {}. You must preserve contracts with affected nodes: {}.",
                            impact["blast_radius"],
                            names.join(", ")
                        );
                        println!("\x1b[31m>> [IMPACT_ENGINE] {} \x1b[0m", blast_radius_info);
                    }
                }
            }

            if let Ok(map_data) = std::fs::read_to_string("repository_analysis/symbol_graph.json") {
                if let Ok(map) = serde_json::from_str::<serde_json::Value>(&map_data) {
                    let mut found = false;
                    if let Some(files) = map.as_array() {
                        for file in files {
                            if let Some(nodes) = file["nodes"].as_array() {
                                for node in nodes {
                                    if node["name"].as_str() == Some(target_func) {
                                        let path = file["file_path"].as_str().unwrap_or("");
                                        let start = node["start_line"].as_u64().unwrap_or(0);
                                        let end = node["end_line"].as_u64().unwrap_or(0);
                                        let ntype = node["node_type"].as_str().unwrap_or("");

                                        println!(
                                            "\x1b[32m>> [AST] Found {} in {} (Lines {}-{})\x1b[0m",
                                            ntype, path, start, end
                                        );

                                        let prompt = format!("MISSION UPDATE (GOD MODE):\n1. You are optimizing the {} `{}`.\n2. DO NOT read any files. The code is located in `{}` exactly from line {} to {}.\n3. IMPACT FORESIGHT: {}\n4. Analyze what it does, optimize or expand its capabilities, and immediately use your `DiffPatch` tool to apply the improvements.\n5. Once patched, run `cargo check` to verify.", ntype, target_func, path, start, end, blast_radius_info);

                                        println!("\x1b[36m>> [ORCHESTRATOR] Fusing Surgical Prompt with Impact Foresight...\x1b[0m");
                                        let _res = client
                                            .post("http://127.0.0.1:3000/api/task")
                                            .header("Authorization", format!("Bearer {}", token))
                                            .json(&serde_json::json!({ "directive": prompt }))
                                            .send()
                                            .await;

                                        found = true;
                                        break;
                                    }
                                }
                            }
                            if found {
                                break;
                            }
                        }
                    }
                    if !found {
                        println!(
                            "\x1b[31m>> [ERROR] Target '{}' not found in AST map.\x1b[0m",
                            target_func
                        );
                    }
                    continue;
                }
            }
        }

        // Default routing for non-/mutate commands
        let _res = client
            .post("http://127.0.0.1:3000/api/task")
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({ "directive": directive }))
            .send()
            .await;

        match _res {
            Ok(response) => {
                if !response.status().is_success() {
                    println!(
                        "\x1b[31m>> [ERROR] Daemon rejected task: {}\x1b[0m",
                        response.status()
                    );
                } else {
                    println!(
                        "\x1b[32m>> [OK] Task dispatched to Background Executive Cortex.\x1b[0m"
                    );
                }
            }
            Err(e) => println!("\x1b[31m>> [ERROR] Failed to contact Daemon: {}\x1b[0m", e),
        }
    }
    Ok(())
}
