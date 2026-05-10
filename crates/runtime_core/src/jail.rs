use std::process::{Command, Output};
use std::io::Result;

pub struct IsolationJail;

impl IsolationJail {
    // Dynamically detect if we are running in the Motorola/Termux environment
    fn is_termux() -> bool {
        std::env::var("PREFIX").map(|p| p.contains("com.termux")).unwrap_or(false)
    }

    pub fn execute(command: &str) -> Result<Output> {
        let cmd = command.to_lowercase();
        
        // 1. The Emancipation List: Native execution for safe Rust/Cargo pipeline ops
        let host_allowed = [
            "cargo ", "rustc ", "ls ", "cat ", "grep ", "find ", "git ", "pwd", "echo "
        ];
        
        if host_allowed.iter().any(|&prefix| cmd.starts_with(prefix)) {
            return Command::new("sh").arg("-c").arg(command).output();
        }

        // 2. Universal Hardware Sandboxing
        if Self::is_termux() {
            // Termux Node: PRoot Jail
            // Isolates the filesystem root strictly to the current workspace
            Command::new("proot")
                .args([
                    "-r", ".", 
                    "-w", "/", 
                    "-b", "/data/data/com.termux/files/usr:/usr",
                    "-b", "/data/data/com.termux/files/usr/bin:/bin",
                    "/bin/sh", "-c", command
                ])
                .output()
        } else {
            // Crostini Node: Podman Hardened Container
            // Enforces no-network, read-only root, and drops all Linux capabilities
            Command::new("podman")
                .args([
                    "run", "--rm", 
                    "--network=none", 
                    "--read-only",
                    "--security-opt", "no-new-privileges",
                    "--cap-drop=ALL",
                    "-v", ".:/workspace:rw", 
                    "-w", "/workspace", 
                    "alpine:latest", "sh", "-c", command
                ])
                .output()
        }
    }
}
