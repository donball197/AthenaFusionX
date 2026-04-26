// src/cli.rs — v4.7 [ASYNC_INQUISITOR_FIX]
use crate::inquisitor::Inquisitor;
use crate::task_manager;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::process::Command;

async fn git_context() -> String {
    let output = Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"]).output().await;
    let branch = output.ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_else(|| "dev".into());
    
    let output_log = Command::new("git").args(["log", "--oneline", "-5"]).output().await;
    let log = output_log.ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_else(|| "(no commits)\n".into());

    let output_stat = Command::new("git").args(["status", "--short"]).output().await;
    let status = output_stat.ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();

    format!(
        "BRANCH: {}\nRECENT COMMITS:\n{}\nPENDING CHANGES:\n{}",
        branch.trim(),
        log.trim(),
        if status.trim().is_empty() { "(clean)" } else { status.trim() }
    )
}

pub async fn inject_workspace_dna(directive: &str) -> String {
    let mut modified = directive.trim().to_string();
    if modified.is_empty() { return String::new(); }

    let lower = modified.to_lowercase();
    if lower.contains("diagnose") || lower.contains("evolve") || lower.contains("suture") {
        println!("\x1b[33m>> [INQUISITOR] Intercepting Evolutionary Directive. Scanning substrate...\x1b[0m");
        // ⚡ SUTURE: Await the new async Inquisitor
        let diag = Inquisitor::diagnose_workspace().await.unwrap_or_else(|e| format!("DIAGNOSTIC_FAULT: {}", e));
        modified = format!("{}\n\n[INQUISITOR_REPORT]\n{}\n[END_REPORT]", modified, diag);
    }

    let schema = r#"[NEXUS SOVEREIGN KERNEL — v4.7 DEVOPS FACTORY]
You are an autonomous DevOps agent on ARM64 Chromebook (4GB RAM).
You manage source code, the Zig/Rust VMO microkernel, and Git.

OUTPUT RULES (MANDATORY):
 Reasoning: ONLY inside <THOUGHT></THOUGHT> (max 3 sentences)
 Actions:   ONLY inside <ACTION></ACTION> as a valid JSON array
 Completion: [{"type": "halt"}]

AVAILABLE ACTIONS:
 {"type": "read_file",          "path": "src/module.rs"}
 {"type": "write_file",         "path": "src/module.rs", "content": "..."}
 {"type": "execute_command",    "command": "cargo check"}
 {"type": "lazarus_snapshot",   "path": "src/main.rs", "tag": "tag"}
 {"type": "search_memory",      "query": "term"}
 {"type": "export_report",      "title": "Title", "content": "..."}
 {"type": "analyze_image",      "path": "ui/dist/shot.png", "query": "What is broken?"}
 {"type": "synthesize_wasm",    "code": "pub extern \"C\" fn my_fn(x: f32) -> f32 { x * 2.0 }"}
 {"type": "wasm_execute_local", "func_name": "my_fn", "input": 10.0}
 {"type": "wasm_load",          "name": "cell_name", "func_name": "my_fn", "input": 10.0}
 {"type": "vmo_create",         "size": 65536}
 {"type": "vmo_write",          "handle": "0x1a", "data": "payload"}
 {"type": "calculate_entropy",  "handle": "0x1a"}
 {"type": "set_stress_test",    "enabled": true}
 {"type": "propose_ui_module",  "id": "my_module", "label": "My Module", "kind": "terminal", "source": "...", "directive": "run cargo check"}
 {"type": "halt"}

FACTORY PROTOCOL (IN ORDER):
 1. NEVER bundle dependent actions. If you need to search memory, output ONLY `search_memory`, wait for the result in the next turn, and THEN take the next action.
 2. write_file → execute_command cargo check — every write.
 3. halt ONLY when directive is satisfied.

ANTI-LOOP RULE: If MISSION HISTORY shows the same action 3 times, halt."#;

    let git = git_context().await;
    format!("SYSTEM_CONTEXT: [SOVEREIGN v4.7]\n{}\n\nGIT_STATE:\n{}\n\nDIRECTIVE: {}", schema, git, modified)
}

pub async fn run_cli_loop(state: Arc<crate::server::AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    let mut rx = state.tx.subscribe();

    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                if let Some(text) = val["payload"].as_str() {
                    match val["type"].as_str().unwrap_or("") {
                        "NEURAL_STREAM" => { print!("{}", text); io::stdout().flush().ok(); }
                        "NEURAL_THOUGHT" => { println!("\x1b[34m>> [THOUGHT] {}\x1b[0m", text.trim()); }
                        "TASK_STATUS" => { println!("\n\x1b[32m>> [STATUS] {}\x1b[0m", text); }
                        "LOG_EVENT" => { println!("\x1b[33m{}\x1b[0m", text); io::stdout().flush().ok(); }
                        _ => {}
                    }
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
            if stdin.read_line(&mut line)? == 0 { break; }
            if line.trim().is_empty() { break; }
            directive_buffer.push_str(&line);
        }

        let directive = directive_buffer.trim().to_string();
        if directive.is_empty() { continue; }
        if directive == "exit" || directive == "quit" { break; }

        let dna = inject_workspace_dna(&directive).await;

        task_manager::send_task(
            state.orchestrator.clone(),
            &state.client,
            &state.api_key,
            &dna,
            state.memory.clone(),
            state.vmo_state.clone(),
            state.tx.clone(),
            state.dist.clone(),
        ).await;

        println!();
    }
    Ok(())
}
