import difflib

TARGET = "crates/runtime_core/src/podman.rs"
EXPECTED = '''use std::io::{Error, ErrorKind, Result};
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
                    "--network=host", // \u26a1 SUTURE: Bypasses container network isolation
                    "--dns=8.8.8.8",  // \u26a1 SUTURE: Ensures Maven Central DNS resolution
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
        // \u26a1 REGRESSION FIX: Explicitly check for command success and expose stderr
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(Error::other(
                format!("Command failed with status: {}\\nSTDERR:\\n{}", out.status, stderr)
            ));
        }
        Ok(out)
    }
}'''

with open(TARGET, "r", encoding="utf-8") as f:
    actual = f.read()

diff = list(difflib.unified_diff(
    EXPECTED.splitlines(keepends=True),
    actual.splitlines(keepends=True),
    fromfile="expected",
    tofile="actual_file",
    lineterm=""
))
if not diff:
    print("NO DIFFERENCE -- files are identical, anchor should have matched. Something else is wrong.")
else:
    print("DIFFERENCES:")
    for line in diff:
        print(line)
