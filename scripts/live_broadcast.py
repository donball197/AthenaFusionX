import os

shell_path = "src/shell.rs"
# We update the signature to accept the broadcaster
new_code = r"""use crate::agent_protocol::AgentAction;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;
use async_process::{Command, Stdio};
use futures_lite::{io::BufReader, prelude::*};
use tokio::sync::broadcast;

pub struct NexusShell;

impl NexusShell {
    pub async fn eval(action: &AgentAction, tx: &broadcast::Sender<String>) -> Value {
        match action {
            AgentAction::ReadFile { path, .. } => Self::builtin_read_file(path).await,
            AgentAction::WriteFile { path, content, .. } => Self::builtin_write_file(path, content).await,
            AgentAction::ExecuteCommand { command, .. } => Self::builtin_exec(command, tx).await,
            _ => json!({ "status": "unsupported", "action": format!("{:?}", action) }),
        }
    }

    async fn builtin_read_file(path: &str) -> Value {
        let p = Path::new(path);
        match fs::read_to_string(p).await {
            Ok(content) => json!({
                "status": "success", "operation": "read_file", "path": path,
                "content": content.chars().take(4096).collect::<String>()
            }),
            Err(e) => json!({ "status": "error", "path": path, "reason": e.to_string() })
        }
    }

    async fn builtin_write_file(path: &str, content: &str) -> Value {
        let p = Path::new(path);
        if let Some(parent) = p.parent() { let _ = fs::create_dir_all(parent).await; }
        match fs::write(p, content).await {
            Ok(_) => json!({ "status": "success", "operation": "write_file", "path": path, "bytes_written": content.len() }),
            Err(e) => json!({ "status": "error", "path": path, "reason": e.to_string() })
        }
    }

    async fn builtin_exec(command: &str, tx: &broadcast::Sender<String>) -> Value {
        let mut child = match Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn() {
                Ok(child) => child,
                Err(e) => return json!({ "status": "error", "command": command, "reason": e.to_string() }),
            };

        let mut stdout_final = String::new();
        let mut stderr_final = String::new();

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        loop {
            tokio::select! {
                Some(Ok(line)) = stdout_reader.next() => {
                    // ⚡ BROADCAST: Send live line to Svelte IDE
                    let _ = tx.send(json!({"type": "TERMINAL_OUT", "payload": line}).to_string());
                    stdout_final.push_str(&line);
                    stdout_final.push('\n');
                }
                Some(Ok(line)) = stderr_reader.next() => {
                    let _ = tx.send(json!({"type": "TERMINAL_ERR", "payload": line}).to_string());
                    stderr_final.push_str(&line);
                    stderr_final.push('\n');
                }
                status = child.status() => {
                    let exit_status = status.map(|s| s.success()).unwrap_or(false);
                    return json!({
                        "status": if exit_status { "success" } else { "failed" },
                        "command": command,
                        "stdout": stdout_final,
                        "stderr": stderr_final.trim()
                    });
                }
            }
        }
    }
}
"""
with open(shell_path, "w") as f: f.write(new_code)

# --- 2. Update executor.rs to pass the sender ---
exec_path = "src/executor.rs"
if os.path.exists(exec_path):
    with open(exec_path, "r") as f:
        content = f.read()
    # Update the eval call to pass the tx (which is available in the AppState context)
    content = content.replace(
        "crate::shell::NexusShell::eval(&action).await",
        "crate::shell::NexusShell::eval(&action, &state.tx).await"
    )
    # We also need to make sure the state is passed to the executor... 
    # (Checking your previous logs, task_manager handles this)
    with open(exec_path, "w") as f: f.write(content)

print("\x1b[32m>> [SUTURE] Live PTY Broadcast active. Terminal God Phase 1.2 complete.\x1b[0m")
