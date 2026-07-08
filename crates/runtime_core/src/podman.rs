use std::io::{Error, ErrorKind, Result};
use std::process::{Command, Output};

pub struct PodmanShield;
impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        // Gate through ArbiterAgent's dangerous-pattern scanner FIRST.
        // This was dropped by a later autonomous commit (eb3b6a78) that
        // also added --network=host and --dns=8.8.8.8 to the dead
        // read-only/no-toolchain fallback below -- restoring the gate
        // AND removing the podman fallback entirely closes both.
        if let Err(e) =
            crate::arbiter_agent::ArbiterAgent::validate_action("execute_command", command)
        {
            return Err(Error::new(ErrorKind::PermissionDenied, e));
        }

        let cmd = command.to_lowercase();

        // 1. The Ironclad Axiom Block (Never allow self-deletion)
        if cmd.contains("rm -rf src") || cmd.contains("rm -rf .git") || cmd.contains("rm -rf /") {
            return Err(Error::new(ErrorKind::PermissionDenied, "AXIOM_BLOCK: Destructive path."));
        }

        // The old podman/alpine fallback was read-only with no Rust
        // toolchain and (as of eb3b6a78) full host networking -- worst of
        // both worlds: broken for real work, and not actually sandboxed.
        // Arbiter gates dangerous patterns above; run validated commands
        // directly on host.
        let out = Command::new("sh").arg("-c").arg(command).output()?;

        // REGRESSION FIX: Explicitly check for command success and expose stderr
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(Error::other(format!(
                "Command failed with status: {}\nSTDERR:\n{}",
                out.status, stderr
            )));
        }

        Ok(out)
    }
}
