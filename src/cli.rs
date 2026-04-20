use std::io::{self, Write};
use std::sync::Arc;
use crate::server::AppState;
use crate::task_manager;

pub fn inject_workspace_dna(directive: &str) -> String {
    let directive = directive.trim();
    if directive.is_empty() { return String::new(); }

    let schema = r#"
[NEXUS SOVEREIGN KERNEL — v2.5 FINAL — STRICT COMPLIANCE]
You are NEXUS Ghost on ARM64 with Zircon-style VMO kernel.

OUTPUT RULES (VIOLATION = FAILURE):
- Reasoning: ONLY inside <THOUGHT></THOUGHT> — max 3 short sentences. No bullets, no extra text.
- Actions: ONLY inside <ACTION></ACTION> — EXACTLY ONE action as a valid JSON array.
- Nothing else may appear outside these two tags. No explanations, no markdown.
- Extract "0x..." handle ONLY from the most recent EXECUTOR line (e.g. "VMO_CREATE: 0xc90c8", "VMO_CLONE_SUCCESS: ... Child 0xca0c9").
- If no 0x handle is visible, output EXACTLY: [{"type": "get_vmo_state"}]
- Never repeat get_vmo_state.
- Use "halt" ONLY as the final action in the very last step (Step 7).

CURRENT_VMO_STATE is injected every turn — use it to find handles.

AVAILABLE ACTIONS:
{"type": "vmo_create"}
{"type": "vmo_clone", "parent_handle": "0x..."}
{"type": "vmo_write", "handle": "0x...", "data": "string"}
{"type": "vmo_read", "handle": "0x..."}
{"type": "get_vmo_state"}
{"type": "export_report", "title": "Title", "content": "body"}
{"type": "halt"}

Follow numbered steps strictly, one action per turn.
"#;

    format!(
        "SYSTEM_CONTEXT: [SOVEREIGN_MODE v2.5 FINAL — STRICT COMPLIANCE REQUIRED]\n{}\n\nDIRECTIVE: {}",
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
                match val["type"].as_str().unwrap_or("") {
                    "NEURAL_STREAM" => {
                        if let Some(text) = val["payload"].as_str() {
                            print!("{}", text);
                            let _ = io::stdout().flush();
                        }
                    }
                    "NEURAL_THOUGHT" => {
                        if let Some(text) = val["payload"].as_str() {
                            let clean = text.trim();
                            if !clean.is_empty() {
                                println!("\x1b[34m>> [THOUGHT] {}\x1b[0m", clean);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    loop {
        print!("\r\x1b[K\x1b[1;32mNEXUS_OS >> \x1b[0m");
        let _ = io::stdout().flush();
        input.clear();
        if stdin.read_line(&mut input)? == 0 { continue; }
        let directive = input.trim().to_string();
        if directive.is_empty() { continue; }
        if directive == "exit" || directive == "quit" { break; }

        let enriched = inject_workspace_dna(&directive);
        let s = state.clone();
        task_manager::send_task(
            s.agent.clone(), &s.client, &s.api_key, &enriched,
            s.memory.clone(), s.vmo_state.clone(), s.tx.clone(),
        ).await;
        println!();
    }
    Ok(())
}
