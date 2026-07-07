use std::io::{Error, ErrorKind, Result};
use std::process::{Command, Output};
use crate::arbiter_agent::ArbiterAgent;

pub struct PodmanShield;
impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        // ⚡ Defense-in-Depth: Run the command through the Arbiter's neural guardrails
        if let Err(e) = ArbiterAgent::validate_action("execute_command", command) {
            return Err(Error::new(ErrorKind::PermissionDenied, e));
        }

        let cmd = command.to_lowercase();

        // 1. The Ironclad Axiom Block (Never allow self-deletion)
        if cmd.contains("rm -rf src") || cmd.contains("rm -rf .git") || cmd.contains("rm -rf /") {
            return Err(Error::new(ErrorKind::PermissionDenied, "AXIOM_BLOCK: Destructive path."));
        }

        // 2. The Emancipation List (Allow safe host commands)
        let host_allowed = [
            "cargo", "ls", "cat", "python3", "python", "grep", "find",
            "git", "pwd", "echo", "sed", "awk"
        ];
        
        let cmd_base = cmd.split_whitespace().next().unwrap_or("");

        // ⚡ FIX: Use bash with strict flags to prevent profile-based hijacking
        let out = if host_allowed.contains(&cmd_base) {
            Command::new("bash")
                .arg("--noprofile")
                .arg("--norc")
                .arg("-c")
                .arg(command)
                .output()?
        } else {
            // 3. Fallback Sandbox (Hardened to read-only, NO network access)
            Command::new("podman")
                .args([
                    "run",
                    "--rm",
                    "--network", "none", // ⚡ Isolate untrusted execution completely
                    "-v",
                    ".:/workspace:ro", 
                    "-w",
                    "/workspace",
                    "alpine:latest",
                    "sh",
                    "-c",
                    command,
                ])
                .output()?
        };

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(Error::other(
                format!("Command failed with status: {}\nSTDERR:\n{}", out.status, stderr)
            ));
        }

        Ok(out)
    }
}
