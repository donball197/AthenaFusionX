use futures_util::StreamExt;
use reqwest::Client;
use std::io::{self, Write};
use std::process::Command;
use tokio_tungstenite::connect_async;

/// Detect a bash heredoc opener on this line and return its delimiter.
/// Recognizes `<<WORD`, `<<'WORD'`, `<<"WORD"`, and `<<-WORD`. Skips
/// here-strings (`<<<`), shift-assign (`<<=`), and arithmetic shifts
/// (`a << b`) so ordinary Rust code pasted into the prompt is not
/// mistaken for a heredoc opener.
fn split_heredoc_delimiter(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'<' && bytes[i + 1] == b'<' {
            if i + 2 < bytes.len() && (bytes[i + 2] == b'<' || bytes[i + 2] == b'=') {
                i += 2;
                continue;
            }
            let preceded_by_ident = i > 0 && {
                let p = bytes[i - 1];
                p.is_ascii_alphanumeric() || p == b'_'
            };
            if preceded_by_ident {
                i += 2;
                continue;
            }
            let mut j = i + 2;
            if j < bytes.len() && bytes[j] == b'-' {
                j += 1;
            }
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let quote = if j < bytes.len() && (bytes[j] == b'\'' || bytes[j] == b'"') {
                let q = bytes[j];
                j += 1;
                Some(q)
            } else {
                None
            };
            let start = j;
            while j < bytes.len() {
                let c = bytes[j];
                if let Some(q) = quote {
                    if c == q {
                        break;
                    }
                } else if !(c.is_ascii_alphanumeric() || c == b'_') {
                    break;
                }
                j += 1;
            }
            if j > start {
                let delim = String::from_utf8_lossy(&bytes[start..j]).into_owned();
                let has_letter = delim.chars().any(|c| c.is_ascii_alphabetic());
                let upper_ok = quote.is_some()
                    || delim
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                if delim.len() >= 2 && has_letter && upper_ok {
                    return Some(delim);
                }
            }
        }
        i += 1;
    }
    None
}

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

    let ws_url: &'static str = Box::leak(
        format!("ws://127.0.0.1:3000/ws/telemetry?token={}", token.trim()).into_boxed_str(),
    );

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

    // If a directive is supplied through argv, dispatch it directly.
    // With no argv directive, preserve the existing interactive EOF workflow.
    let argv_directive = std::env::args().skip(1).collect::<Vec<_>>();
    let single_shot = !argv_directive.is_empty();
    let mut pending_directive =
        if argv_directive.is_empty() { None } else { Some(argv_directive.join(" ")) };

    loop {
        let directive = if let Some(arg_directive) = pending_directive.take() {
            arg_directive
        } else {
            print!(
                "\n\x1b[1;32mnexus>\x1b[0m (Type prompt, then 'EOF' on a new line to submit)\n> "
            );
            io::stdout().flush()?;

            let mut input = String::new();
            let mut pending_heredoc: Option<String> = None;
            loop {
                let mut line = String::new();
                let bytes_read = stdin.read_line(&mut line)?;
                if bytes_read == 0 {
                    break;
                }
                let trimmed = line.trim();

                // Inside a pending heredoc: only the exact delimiter closes
                // it. A bare `EOF` line inside `cat >f <<'EOF' ... EOF` is
                // script content, not the CLI submit sentinel.
                if let Some(delim) = pending_heredoc.as_deref() {
                    input.push_str(&line);
                    if trimmed == delim {
                        pending_heredoc = None;
                    }
                    continue;
                }

                // Heredoc opener on this line: enter heredoc mode.
                if let Some(delim) = split_heredoc_delimiter(&line) {
                    pending_heredoc = Some(delim);
                    input.push_str(&line);
                    continue;
                }

                // Submit sentinel (only when no heredoc is pending).
                if trimmed == "EOF" {
                    break;
                }
                input.push_str(&line);
            }

            input.trim().to_string()
        };

        let directive = directive.trim();
        if directive.is_empty() {
            if argv_directive.is_empty() {
                continue;
            }
            break;
        }
        if directive == "/exit" || directive == "/quit" {
            break;
        }

        if directive.starts_with("/mutate ") {
            let rest = directive.strip_prefix("/mutate ").unwrap().trim();
            let mut parts = rest.splitn(2, |c: char| c == '\n' || c.is_whitespace());
            let target_func = parts.next().unwrap_or("").trim();
            let instruction_raw = parts.next().unwrap_or("").trim();
            let user_instruction = if instruction_raw.is_empty() {
                "Analyze what it does, optimize or expand its capabilities"
            } else {
                instruction_raw
            };

            println!(
                "\x1b[33m>> [AST] Locating '{}' in repository_analysis/symbol_graph.json...\x1b[0m",
                target_func
            );

            // 1. Run the Impact Engine silently
            println!("\x1b[35m>> [AST] Refreshing Symbol Graph to catch new files...\x1b[0m");
            let _ = Command::new("cargo").args(["run", "--bin", "repository_planner"]).output();
            let _ = Command::new("cargo").args(["run", "--bin", "symbol_graph"]).output();
            println!("\x1b[35m>> [IMPACT_ENGINE] Calculating Blast Radius...\x1b[0m");
            let _ = Command::new("cargo")
                .args([
                    "run",
                    "--bin",
                    "impact_analyzer",
                    "--",
                    "--target",
                    target_func,
                    "--symbol-graph",
                    "repository_analysis/symbol_graph.json",
                    "--symbol-graph",
                    "repository_analysis/symbol_graph.json",
                    "--output",
                    "repository_analysis/impact/latest_report.json",
                ])
                .output();

            let mut blast_radius_info = String::new();
            if let Ok(impact_data) =
                std::fs::read_to_string("repository_analysis/impact/latest_report.json")
            {
                if let Ok(impact) = serde_json::from_str::<serde_json::Value>(&impact_data) {
                    if let Some(nodes) = impact["critical_blast_radius"].as_array() {
                        let names: Vec<&str> = nodes.iter().filter_map(|n| n.as_str()).collect();
                        blast_radius_info = format!(
                            "WARNING: This mutation has a blast radius of {}. You must preserve contracts with affected nodes: {}.",
                            impact["blast_radius_score"],
                            names.join(", ")
                        );
                        println!("\x1b[31m>> [IMPACT_ENGINE] {} \x1b[0m", blast_radius_info);
                    }
                }
            }

            if let Ok(map_data) = std::fs::read_to_string("repository_analysis/symbol_graph.json") {
                if let Ok(map) = serde_json::from_str::<serde_json::Value>(&map_data) {
                    let mut found = false;
                    let target_suffix = format!("::{}", target_func);
                    let empty_vec = vec![];
                    let nodes = map
                        .get("nodes")
                        .and_then(|n| n.as_array())
                        .unwrap_or_else(|| map.as_array().unwrap_or(&empty_vec));

                    for node in nodes {
                        let id = node.get("id").and_then(|i| i.as_str()).unwrap_or("");
                        let name = node.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        if id.ends_with(&target_suffix) || name == target_func {
                            let path = node
                                .get("file_path")
                                .or_else(|| node.get("path"))
                                .and_then(|p| p.as_str())
                                .unwrap_or("");
                            let start =
                                node.get("start_line").and_then(|s| s.as_u64()).unwrap_or(1);
                            let end = node.get("end_line").and_then(|e| e.as_u64()).unwrap_or(50);
                            let ntype = node
                                .get("kind")
                                .or_else(|| node.get("node_type"))
                                .and_then(|k| k.as_str())
                                .unwrap_or("Function");

                            println!(
                                "\x1b[32m>> [AST] Found {} in {} (Lines {}-{})\x1b[0m",
                                ntype, path, start, end
                            );

                            let prompt = format!(
                                "MISSION UPDATE (GOD MODE):\n1. You are optimizing the {} `{}`.\n2. DO NOT read any files. The code is located in `{}` exactly from line {} to {}.\n3. IMPACT FORESIGHT: {}\n4. INSTRUCTION: {}\n5. Immediately use your `DiffPatch` tool to apply the improvements.\n6. Once patched, run `cargo check` to verify.",
                                ntype, target_func, path, start, end, blast_radius_info, user_instruction
                            );

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
                    if !found {
                        println!(
                            "\x1b[31m>> [ERROR] Target '{}' not found in AST map.\x1b[0m",
                            target_func
                        );
                    }

                    if single_shot {
                        break;
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

        // argv invocation is single-shot; interactive mode remains persistent.
        if single_shot {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod heredoc_delimiter_tests {
    use super::split_heredoc_delimiter;
    fn d(line: &str) -> Option<String> {
        split_heredoc_delimiter(line)
    }
    #[test]
    fn recognizes_real_openers() {
        assert_eq!(d("cat > f <<EOF"), Some("EOF".to_string()));
        assert_eq!(d("cat > f <<-EOF"), Some("EOF".to_string()));
        assert_eq!(d("printf 'test' <<'CANARY'"), Some("CANARY".to_string()));
        assert_eq!(d("cat <<\"END_OF_FILE\""), Some("END_OF_FILE".to_string()));
    }
    #[test]
    fn ignores_shifts_here_strings_and_prose() {
        assert_eq!(d("let x = 1 << 10;"), None);
        assert_eq!(d("let y = a<<b;"), None);
        assert_eq!(d("cat <<< word"), None);
        assert_eq!(d("x <<= 2"), None);
        assert_eq!(d("use <<end for lowercase"), None);
        assert_eq!(d("no opener here"), None);
    }
}
