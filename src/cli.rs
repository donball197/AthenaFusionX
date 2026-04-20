use std::io::{self, Write};
use std::sync::Arc;
use crate::server::AppState;
use crate::task_manager;

pub fn inject_workspace_dna(directive: &str) -> String {
    let directive = directive.trim();
    if directive.is_empty() { return String::new(); }

    let schema = r#"
[NEXUS SOVEREIGN KERNEL — v2.5 FINAL]
You are NEXUS Ghost on ARM64. You have full access to the VMO microkernel and the host substrate.

OUTPUT RULES:
- Reasoning: ONLY inside <THOUGHT></THOUGHT> (max 3 short sentences).
- Actions: ONLY inside <ACTION></ACTION> — one OR MORE actions in the JSON array to sequence commands.
- Extract "0x..." handle ONLY from the most recent EXECUTOR line or CURRENT_VMO_STATE.

AVAILABLE ACTIONS:
{"type": "execute_command", "command": "cargo check"}
{"type": "search_memory", "query": "term"}
{"type": "vmo_create"}
{"type": "vmo_clone", "parent_handle": "0x..."}
{"type": "vmo_write", "handle": "0x...", "data": "string"}
{"type": "vmo_read", "handle": "0x..."}
{"type": "export_report", "title": "Title", "content": "body"}
{"type": "scrape_url", "url": "https://..."}
{"type": "lazarus_snapshot", "path": "src/main.rs", "tag": "backup"}
{"type": "halt"}

"#;

    format!(
        "SYSTEM_CONTEXT: [SOVEREIGN_MODE v2.5]\n{}\n\nDIRECTIVE: {}",
        schema, directive
    )
}

pub async fn run_cli_loop(state: Arc<AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = String::new();
    let mut rx = state.tx.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                if val["type"] == "NEURAL_STREAM" {
                    if let Some(text) = val["payload"].as_str() { print!("{}", text); io::stdout().flush().ok(); }
                } else if val["type"] == "NEURAL_THOUGHT" {
                    if let Some(text) = val["payload"].as_str() { println!("\x1b[34m>> [THOUGHT] {}\x1b[0m", text.trim()); }
                }
            }
        }
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    loop {
        print!("\r\x1b[K\x1b[1;32mNEXUS_OS >> \x1b[0m");
        io::stdout().flush().ok();
        input.clear();
        if stdin.read_line(&mut input)? == 0 { continue; }
        let directive = input.trim();
        if directive.is_empty() { continue; }
        if directive == "exit" || directive == "quit" { break; }
        task_manager::send_task(state.orchestrator.clone(), &state.client, &state.api_key, &inject_workspace_dna(directive), state.memory.clone(), state.vmo_state.clone(), state.tx.clone()).await;
        println!();
    }
    Ok(())
}
