// src/cli.rs — v4.2 NEXUS CLI Library
// Copyright 2026 Donald Ball — Apache 2.0

use crate::server::AppState;
use crate::task_manager;
use std::io::{self, Write};
use std::process::Command;
use std::sync::Arc;

fn git_context() -> String {
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown\n".into())
        .trim()
        .to_string();

    let log = Command::new("git")
        .args(["log", "--oneline", "-5"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "(no commits)\n".into());

    let status = Command::new("git")
        .args(["status", "--short"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    format!(
        "BRANCH: {}\nRECENT COMMITS:\n{}\nPENDING CHANGES:\n{}",
        branch,
        log.trim(),
        if status.trim().is_empty() { "(clean)" } else { status.trim() }
    )
}

pub fn inject_workspace_dna(directive: &str) -> String {
    let directive = directive.trim();
    if directive.is_empty() {
        return String::new();
    }

    let schema = r#"[NEXUS SOVEREIGN KERNEL — v4.4 DEVOPS FACTORY]
You are an autonomous DevOps agent on ARM64 Chromebook (4GB RAM).
You manage source code, the Zig/Rust VMO microkernel, and Git.

OUTPUT RULES (MANDATORY):
  Reasoning: ONLY inside <THOUGHT></THOUGHT> (max 3 sentences)
  Actions:   ONLY inside <ACTION></ACTION> as a valid JSON array
  Completion: [{"type": "halt"}]

AVAILABLE ACTIONS:
  {"type": "read_file",            "path": "src/module.rs"}
  {"type": "write_file",           "path": "src/module.rs", "content": "..."}
  {"type": "execute_command",      "command": "cargo check"}
  {"type": "git_action",           "command": "status|commit|branch|checkout|log|diff|push", "message": "..."}
  {"type": "lazarus_snapshot",     "path": "src/", "tag": "tag"}
  {"type": "search_memory",        "query": "term"}
  {"type": "export_report",        "title": "Title", "content": "..."}
  {"type": "synthesize_wasm",      "code": "pub extern \"C\" fn my_fn(x: f32) -> f32 { x * 2.0 }"}
  {"type": "wasm_execute_local",   "func_name": "my_fn", "input": 10.0}
  {"type": "wasm_store",           "name": "cell_name", "func_name": "my_fn"}
  {"type": "wasm_load",            "name": "cell_name", "func_name": "my_fn", "input": 10.0}
  {"type": "execute_distributed_wasm", "wasm_b64": "WASM_CELL_READY", "func_name": "my_fn", "input": 10.0}
  {"type": "halt"}

WASM TWO-STEP PROTOCOL (ALWAYS follow this order):
  Step 1: {"type": "synthesize_wasm", "code": "pub extern \"C\" fn my_fn(x: f32) -> f32 { ... }"}
  Step 2: {"type": "wasm_execute_local", "func_name": "my_fn", "input": 10.0}
  DO NOT use synthesize_wasm more than once per mission.
  DO NOT try to extract or embed the base64 payload — use wasm_execute_local instead.
  When you see WASM_SYNTHESIS_COMPLETE in history, the cell is ready — go straight to Step 2.

FACTORY PROTOCOL (IN ORDER):
  1. ONE lazarus_snapshot at the start — never repeat it.
  2. read_file before write_file — never overwrite without reading first.
  3. write_file → execute_command cargo check — every write, no exceptions.
  4. git_action branch before new features — never commit to main.
  5. halt ONLY when directive is 100% satisfied.

ANTI-LOOP RULE: If your MISSION HISTORY shows the same action 3 times
in a row, you are in a loop. Switch to a different action or halt."#;

    let git = git_context();

    format!(
        "SYSTEM_CONTEXT: [SOVEREIGN_DEVOPS_FACTORY v4.2]\n\
         {}\n\n\
         GIT_STATE:\n{}\n\n\
         DIRECTIVE: {}",
        schema, git, directive
    )
}

pub async fn run_cli_loop(state: Arc<AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    let mut rx = state.tx.subscribe();

    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                match val["type"].as_str().unwrap_or("") {
                    "NEURAL_STREAM" => {
                        if let Some(text) = val["payload"].as_str() {
                            print!("{}", text);
                            io::stdout().flush().ok();
                        }
                    }
                    "NEURAL_THOUGHT" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\x1b[34m>> [THOUGHT] {}\x1b[0m", text.trim());
                        }
                    }
                    "TASK_STATUS" => {
                        if let Some(text) = val["payload"].as_str() {
                            println!("\n\x1b[32m>> [STATUS] {}\x1b[0m", text);
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    loop {
        print!("\r\x1b[K\x1b[1;32mNEXUS_FACTORY >> (paste directive, blank line to execute)\n> \x1b[0m");
        io::stdout().flush().ok();

        let mut directive_buffer = String::new();
        loop {
            let mut line = String::new();
            if stdin.read_line(&mut line)? == 0 {
                break;
            }
            if line.trim().is_empty() {
                break;
            }
            directive_buffer.push_str(&line);
        }

        let directive = directive_buffer.trim().to_string();
        if directive.is_empty() {
            continue;
        }
        if directive == "exit" || directive == "quit" {
            break;
        }

        task_manager::send_task(
            state.orchestrator.clone(),
            &state.client,
            &state.api_key,
            &inject_workspace_dna(&directive),
            state.memory.clone(),
            state.vmo_state.clone(),
            state.tx.clone(),
            state.dist.clone(),
        )
        .await;

        println!();
    }
    Ok(())
}
