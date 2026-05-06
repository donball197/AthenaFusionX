// src/cli.rs — v6.3 [STRICT_JSON_TYPE + GHOST_GUARD]
use crate::task_manager;
use std::io::{self, Write};
use tokio::io::{self as tokio_io, AsyncBufReadExt};

use std::sync::Arc;

pub async fn inject_workspace_dna(directive: &str) -> String {
    if directive.trim().is_empty() {
        return "No directive provided. Halt immediately.".to_string();
    }
    directive.trim().to_string()
}

pub async fn get_execution_law_v6() -> String {
    r#"You are NEXUS — a silent, precise, rule-bound autonomous coding agent.

=== ABSOLUTE OUTPUT RULES (VIOLATION = SYSTEM FAILURE) ===
- Output **ONLY** a valid JSON array of AgentAction objects.
- NO explanations, NO thoughts, NO markdown, NO "I think", NO preamble.
- Response must start with `[` and end with `]`.
- Keep "reasoning" fields very short (max 12 words).

=== ONE LOGICAL STEP PER TURN ===
- Do exactly one meaningful action per response.
- After read_file / diff_patch / write_file / execute_command, stop and wait for next turn.
- If mission is complete based on observations, output only `halt`.

=== EDITING PREFERENCE ===
- Prefer `diff_patch` for small surgical changes.
- Use `patch_function` only when asked for semantic/AST-level function replacement.
- Use `read_file` first when you need to see current code.

=== READ_FILE PROTOCOL ===
- When you see [READ_OK] with --- CONTENT --- block, use that content directly.
- Never re-read the same file if content is already provided.

=== STRICT JSON SCHEMA ===
Every action MUST include the "type" field exactly as shown below:
{ "type": "read_file", "path": "...", "reasoning": "..." }
{ "type": "write_file", "path": "...", "content": "...", "reasoning": "..." }
{ "type": "diff_patch", "path": "...", "search": "...", "replace": "...", "reasoning": "..." }
{ "type": "patch_function", "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." }
{ "type": "execute_command", "command": "...", "reasoning": "..." }
{ "type": "halt", "reasoning": "..." }

Now execute the directive. Output ONLY the JSON array."#.to_string()
}

pub async fn run_cli_loop(state: Arc<crate::server::AppState>) -> io::Result<()> {
    let stdin = tokio_io::stdin();
    let mut reader = tokio_io::BufReader::new(stdin);
    loop {
        print!("\n\x1b[1;32mNEXUS_FACTORY >> \x1b[0m");
        io::stdout().flush().ok();
        let mut input = String::new();
        if reader.read_line(&mut input).await? == 0 { break; }
        let trimmed = input.trim();
        if trimmed.is_empty() { continue; }
        if trimmed == "exit" || trimmed == "quit" {
            println!("\x1b[33m>> [CLI] Graceful shutdown requested.\x1b[0m");
            break;
        }

        let mut rx = state.tx.subscribe();
        let stream_handle = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                    if val["type"] == "NEURAL_STREAM" {
                        if let Some(token) = val["payload"].as_str() {
                            print!("\x1b[38;5;33m{}\x1b[0m", token);
                            std::io::stdout().flush().ok();
                        }
                    }
                }
            }
        });

        task_manager::send_task(
            state.orchestrator.clone(),
            &state.client,
            &state.api_key,
            trimmed,
            state.memory.clone(),
            state.vmo_state.clone(),
            state.tx.clone(),
            state.dist.clone(),
        ).await;

        stream_handle.abort();
    }
    Ok(())
}
