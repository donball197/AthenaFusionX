import os

fpath = "src/shell.rs"
code = r"""use crate::agent_protocol::AgentAction;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct NexusShell;

impl NexusShell {
    pub async fn eval(action: &AgentAction) -> Value {
        match action {
            AgentAction::ReadFile { path, .. } => Self::builtin_read_file(path).await,
            AgentAction::WriteFile { path, content, .. } => Self::builtin_write_file(path, content).await,
            AgentAction::ExecuteCommand { command, .. } => Self::builtin_exec(command).await,
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

    async fn builtin_exec(command: &str) -> Value {
        println!("\n\x1b[1;36m>> [NEXUS_SHELL] Executing: {}\x1b[0m", command);
        
        let mut child = match Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn() 
        {
            Ok(child) => child,
            Err(e) => return json!({ "status": "error", "command": command, "reason": e.to_string() }),
        };

        let stdout = child.stdout.take().expect("Failed to bind stdout");
        let stderr = child.stderr.take().expect("Failed to bind stderr");

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut captured_stdout = String::new();
        let mut captured_stderr = String::new();

        // Stream stdout in real-time
        let stdout_task = tokio::spawn(async move {
            let mut out = String::new();
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                println!("\x1b[37m{}\x1b[0m", line); // Print to terminal instantly
                out.push_str(&line);
                out.push('\n');
            }
            out
        });

        // Stream stderr in real-time
        let stderr_task = tokio::spawn(async move {
            let mut err = String::new();
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                eprintln!("\x1b[31m{}\x1b[0m", line); // Print to terminal instantly in red
                err.push_str(&line);
                err.push('\n');
            }
            err
        });

        if let Ok(out) = stdout_task.await { captured_stdout = out; }
        if let Ok(err) = stderr_task.await { captured_stderr = err; }

        let status = child.wait().await.unwrap_or_else(|_| std::process::ExitStatus::default());

        json!({
            "status": if status.success() { "success" } else { "failed" },
            "command": command,
            "stdout": captured_stdout.trim().chars().take(4096).collect::<String>(),
            "stderr": captured_stderr.trim().chars().take(4096).collect::<String>()
        })
    }
}
"""
with open(fpath, "w") as f:
    f.write(code)
print("\x1b[32m>> [SUTURE] shell.rs upgraded to Real-Time Streaming.\x1b[0m")
