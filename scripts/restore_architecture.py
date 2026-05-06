import os
import re

# 1. RESTORE GHOST DOM PROTOCOL
fpath = "src/agent_protocol.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f: code = f.read()
    if "FetchUiSnapshot" not in code:
        code = code.replace(
            '#[serde(rename = "halt")] Halt,',
            '#[serde(rename = "fetch_ui_snapshot")] FetchUiSnapshot,\n    #[serde(rename = "halt")] Halt,'
        )
        with open(fpath, "w") as f: f.write(code)
        print(">> [SUTURE] Restored FetchUiSnapshot to agent_protocol.rs")

# 2. FORGE THE REAL PODMAN SHIELD
podman_code = """use std::process::{Command, Output};
use std::io::{Error, ErrorKind, Result};

pub struct PodmanShield;

impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        let cmd_lower = command.to_lowercase();
        if cmd_lower.contains("rm -rf src") || cmd_lower.contains("rm -rf .git") || cmd_lower.contains("mkfs") {
            tracing::warn!("[SHIELD] Blocked destructive command: {}", command);
            return Err(Error::new(ErrorKind::PermissionDenied, "AXIOM_BLOCK: Destructive commands forbidden."));
        }
        if cmd_lower.starts_with("cargo ") || cmd_lower.starts_with("ls") || cmd_lower.starts_with("cat") || cmd_lower.starts_with("git ") {
            tracing::info!("[SHIELD] Native execution: {}", command);
            return Command::new("sh").arg("-c").arg(command).output();
        }
        tracing::info!("[SHIELD] Sandboxing: {}", command);
        Command::new("podman")
            .args(["run", "--rm", "--network", "none", "-v", ".:/workspace", "-w", "/workspace", "alpine:latest", "sh", "-c", command])
            .output()
    }
}
"""
with open("src/podman.rs", "w") as f: f.write(podman_code)
print(">> [SUTURE] Restored Tri-State PodmanShield to src/podman.rs")

# 3. FIX AXUM 0.7 TYPE INFERENCE ORDER
fpath = "src/server.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f: code = f.read()
    
    # Remove the premature into_make_service call
    code = re.sub(r"\.into_make_service_with_connect_info::<SocketAddr>\(\)\s*", "", code)
    
    # Inject it correctly into the axum::serve call
    if "axum::serve(listener, app).await" in code:
        code = code.replace(
            "axum::serve(listener, app).await",
            "axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await"
        )
        with open(fpath, "w") as f: f.write(code)
        print(">> [SUTURE] Corrected Axum Router type inference in src/server.rs")

# 4. REMOVE UNUSED IMPORT
fpath = "src/token_ledger.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f: code = f.read()
    code = code.replace("use std::net::IpAddr;\n", "")
    with open(fpath, "w") as f: f.write(code)
    print(">> [SUTURE] Cleaned imports in src/token_ledger.rs")

