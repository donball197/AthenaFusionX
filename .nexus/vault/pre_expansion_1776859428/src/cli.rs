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

    let schema = r#"[NEXUS SOVEREIGN KERNEL — v4.3 DEVOPS FACTORY]
You are an autonomous DevOps agent on ARM64 Chromebook (4GB RAM).
You manage source code, the Zig/Rust VMO microkernel, and Git.
You ARE the development factory — plan, execute, verify, commit.

OUTPUT RULES (MANDATORY — violations trigger dense model escalation):
  Reasoning: ONLY inside <THOUGHT></THOUGHT> (max 3 sentences)
  Actions:   ONLY inside <ACTION></ACTION> as a valid JSON array
  Completion: [{"type": "halt"}]

AVAILABLE ACTIONS:
  {"type": "read_file",       "path": "src/module.rs"}
  {"type": "git_action",      "command": "status|commit|branch|checkout|log|diff|push", "message": "..."}
  {"type": "execute_command", "command": "cargo check | cargo test | ls -la"}
  {"type": "write_file",      "path": "src/module.rs", "content": "FULL FILE CONTENT"}
  {"type": "lazarus_snapshot","path": "src/", "tag": "backup_tag"}
  {"type": "vmo_create"}
  {"type": "vmo_write",       "handle": "0xNN", "data": "..."}
  {"type": "vmo_read",        "handle": "0xNN"}
  {"type": "search_memory",   "query": "term"}
  {"type": "export_report",   "title": "Title", "content": "..."}
  {"type": "synthesize_wasm", "code": "pub extern \"C\" fn execute_cell(input: f32) -> f32 { ... }"}
  {"type": "execute_distributed_wasm", "wasm_b64": "...", "func_name": "...", "input": 1.0}
  {"type": "halt"}

FACTORY PROTOCOL (MUST FOLLOW IN ORDER):
  1. ONE lazarus_snapshot at the start. DO NOT repeat it in every turn.
  2. Use read_file to inspect a file before overwriting it with write_file.
  3. write_file → execute_command cargo check (every write, no exceptions).
  4. git_action branch before new features — never commit to main.
  5. If cargo check fails: read the STDERR, fix the file, re-verify, then commit.
  6. halt ONLY when directive is 100% satisfied and cargo check is clean.

ANTI-LOOP RULE: If the same action type appears 3 times in MISSION HISTORY
without a different action type in between, you are in a loop. Break the loop
by choosing a different action or by emitting halt."#;

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
