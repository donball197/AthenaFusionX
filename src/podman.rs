use std::process::{Command, Output};
use std::io::{Error, ErrorKind, Result};

pub struct PodmanShield;
impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        let cmd = command.to_lowercase();
        if cmd.contains("rm -rf src") || cmd.contains("rm -rf .git") {
            return Err(Error::new(ErrorKind::PermissionDenied, "AXIOM_BLOCK: Destructive path."));
        }
        if cmd.starts_with("cargo ") || cmd.starts_with("ls") || cmd.starts_with("cat") {
            return Command::new("sh").arg("-c").arg(command).output();
        }
        Command::new("podman").args(["run","--rm","--network","none","-v",".:/workspace","-w","/workspace","alpine:latest","sh","-c",command]).output()
    }
}
