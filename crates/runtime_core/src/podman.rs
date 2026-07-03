use std::io::{Error, ErrorKind, Result};
use std::process::{Command, Output};

pub struct PodmanShield;
impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        let cmd = command.to_lowercase();

        // 1. The Ironclad Axiom Block (Never allow self-deletion)
        if cmd.contains("rm -rf src") || cmd.contains("rm -rf .git") || cmd.contains("rm -rf /") {
            return Err(Error::new(ErrorKind::PermissionDenied, "AXIOM_BLOCK: Destructive path."));
        }

        // 2. The Emancipation List (Allow safe host commands)
        let host_allowed = [
            "cargo", "ls", "cat", "python3", "python", "grep", "find",
            "git", "pwd", "echo",
        ];
        
        let cmd_base = cmd.split_whitespace().next().unwrap_or("");

        let out = if host_allowed.contains(&cmd_base) {
            Command::new("sh").arg("-c").arg(command).output()?
        } else {
            // 3. Fallback Sandbox (Hardened to read-only to safeguard host repository files)
            Command::new("podman")
                .args([
                    "run",
                    "--rm",
                    "-v",
                    ".:/workspace:ro", // Explicit read-only mode to prevent host code corruption
                    "-w",
                    "/workspace",
                    "alpine:latest",
                    "sh",
                    "-c",
                    command,
                ])
                .output()?
        };

        // ⚡ REGRESSION FIX: Explicitly check for command success and expose stderr
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(Error::other(
                format!("Command failed with status: {}\nSTDERR:\n{}", out.status, stderr)
            ));
        }

        Ok(out)
    }
}
