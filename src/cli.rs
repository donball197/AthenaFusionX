use std::io::{self, Write};
use std::sync::Arc;
use crate::server::AppState;
use crate::task_manager;

#[derive(Debug, Clone)]
pub enum HostMuscle {
    ChromebookCrostini,
    MotorolaTermux,
    GenericARM64,
}

pub fn detect_host_muscle() -> HostMuscle {
    if std::path::Path::new("/usr/bin/cicerone").exists() {
        HostMuscle::ChromebookCrostini
    } else if std::path::Path::new("/data/data/com.termux").exists() {
        HostMuscle::MotorolaTermux
    } else {
        HostMuscle::GenericARM64
    }
}

pub fn get_muscle_string(muscle: &HostMuscle) -> &'static str {
    match muscle {
        HostMuscle::ChromebookCrostini => "CHROMEBOOK [NATIVE LINUX/CROSTINI]",
        HostMuscle::MotorolaTermux => "MOTOROLA [TERMUX/ANDROID]",
        HostMuscle::GenericARM64 => "GENERIC ARM64 SUBSTRATE",
    }
}

pub fn inject_workspace_dna(directive: &str) -> String {
    let strict_schema = r#"
[STRICT JSON SCHEMA REQUIRED]
You are bound by the Serde AgentAction protocol. You must output actions EXACTLY in this format, including the "type" field.

1. To run a terminal command:
{"type": "execute_command", "command": "your_command_here"}

2. To write or overwrite a file:
{"type": "write_file", "path": "file_path", "content": "file_content"}

3. To log system telemetry:
{"type": "system_status", "data": {"key": "value"}}

4. To stop (MANDATORY immediately after an action):
{"type": "halt"}

DO NOT invent new commands, target keys, or parameters. Use ONLY the exact structures above.
"#;
    format!("SYSTEM_CONTEXT: [SOVEREIGN_MODE]\n{}\n\nDIRECTIVE: {}", strict_schema, directive)
}

pub async fn run_cli_loop(state: Arc<AppState>) -> io::Result<()> {
    // Wait for server logs to settle
    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
    
    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        print!("\r\x1b[K\x1b[1;32mNEXUS_OS >> \x1b[0m");
        io::stdout().flush()?;
        
        input.clear();
        if stdin.read_line(&mut input)? == 0 { break; }
        
        let directive = input.trim();
        if directive.is_empty() { continue; }
        
        if directive == "exit" || directive == "halt" { break; }

        let enriched = inject_workspace_dna(directive);
        let s = state.clone();
        tokio::spawn(async move {
            task_manager::send_task(
                s.agent.clone(),
                &s.client,
                &s.api_key,
                &enriched,
                s.memory.clone(),
                s.tx.clone()
            ).await;
        });
    }
    Ok(())
}
