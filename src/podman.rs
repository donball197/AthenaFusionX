use anyhow::Result;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct PodmanShield;

impl PodmanShield {
    const THREAT_SIGNATURES: &'static [&'static str] = &[
        "MALICIOUS_INTENT",
        "PERMISSION_DENIED",
        "SEGFAULT",
        "CORE_DUMP",
        "FATAL_ERROR",
        "OVERFLOW",
        "BACKDOOR",
    ];

    pub async fn execute_isolated(directive: &str, workspace_path: &str) -> Result<String> {
        println!(">> [SHIELD] Opening Async IPC Channel to Bio-Dome...");

        let mut child = Command::new("podman")
            .args([
                "run", "--rm", "--network", "none", "--memory", "512m",
                "-v", &format!("{}:/workspace:z", workspace_path),
                "-w", "/workspace",
                "docker.io/library/rust:alpine",
                "sh", "-c", directive,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let mut reader = BufReader::new(stdout).lines();
        let mut output_buffer = String::new();

        while let Ok(Some(line)) = reader.next_line().await {
            println!(">> [BIO-DOME]: {}", line);

            if Self::THREAT_SIGNATURES.iter().any(|&sig| line.contains(sig)) {
                println!("!! [ARBITER] Threat signature detected: {}. Severing IPC.", line);
                let _ = child.kill().await;
                return Err(anyhow::anyhow!("SHIELD_BREACH: Intercepted unsafe execution."));
            }

            output_buffer.push_str(&line);
            output_buffer.push('\n');
        }

        let status = child.wait().await?;
        if status.success() {
            Ok(output_buffer)
        } else {
            Err(anyhow::anyhow!("SHIELD_FAULT: Exit status {}", status))
        }
    }
}
