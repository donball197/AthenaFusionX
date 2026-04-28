use crate::task_manager;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::process::Command;

pub async fn inject_workspace_dna(directive: &str) -> String {
    format!(r#"[NEXUS EXECUTION LAW v6]
MANDATORY: Output ONLY a valid JSON array. No explanations.
Rules:
- If you start repeating, immediately output ] and stop.
- Never repeat a phrase more than twice.
- Every write_file MUST be followed by "cargo check".
- Max 5 actions.

DIRECTIVE:
{}"#, directive)
}

pub async fn run_cli_loop(state: Arc<crate::server::AppState>) -> io::Result<()> {
    let stdin = io::stdin();
    loop {
        print!("\x1b[1;32mNEXUS_FACTORY >> \x1b[0m");
        io::stdout().flush().ok();
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let d = inject_workspace_dna(input.trim()).await;
        task_manager::send_task(state.orchestrator.clone(), &state.client, &state.api_key, &d, state.memory.clone(), state.vmo_state.clone(), state.tx.clone(), state.dist.clone()).await;
    }
}
