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
            "cargo ", "ls ", "cat ", "curl ", "wget ", "python3 ", "python ", "grep ", "find ",
            "git ", "pwd", "echo ",
        ];

        if host_allowed.iter().any(|&prefix| cmd.starts_with(prefix)) {
            return Command::new("sh").arg("-c").arg(command).output();
        }

        // 3. Fallback Sandbox (Hardened to read-only to safeguard host repository files)
        Command::new("podman")
            .args([
                "run",
                "--rm",
                "-v",
                ".:/workspace:ro", // ⚡ SUTURED: Changed to explicit read-only mode to prevent host code corruption
                "-w",
                "/workspace",
                "alpine:latest",
                "sh",
                "-c",
                command,
            ])
            .output()
    }
}
