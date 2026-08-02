use std::io::{Error, ErrorKind, Result};
use std::process::{Command, Output};

pub struct PodmanShield;
impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        if let Err(e) =
            crate::arbiter_agent::ArbiterAgent::validate_action("execute_command", command)
        {
            return Err(Error::new(ErrorKind::PermissionDenied, e));
        }
        let mut child = Command::new("sh");
        child.arg("-c").arg(command);
        for (key, value) in std::env::vars() {
            if key != "CARGO_BUILD_TARGET" {
                child.env(key, value);
            }
        }
        child.env_remove("CARGO_BUILD_TARGET");
        let out = child.output()?;

        // REGRESSION FIX (reapplied): explicitly check command success and
        // expose stderr. Without this, a failing command (e.g. a broken
        // `cargo check`) is indistinguishable from a successful one -- both
        // get tagged [CMD_OK] by the executor, which the Cortical Loop's
        // is_terminal_cmd heuristic then treats as proof of success.
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
