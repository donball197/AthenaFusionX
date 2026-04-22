/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
use anyhow::Result;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct PodmanShield;

impl PodmanShield {
    pub async fn spawn_expert_drone(drone_type: &str, workspace: &str) -> anyhow::Result<String> {
        let (image, name) = match drone_type {
            "secops" => ("docker.io/library/alpine:latest", "nexus_secops"),
            "devops" => ("docker.io/library/rust:alpine", "nexus_devops"),
            _ => ("docker.io/library/alpine:latest", "nexus_generic"),
        };

        let _ = tokio::process::Command::new("podman").args(["rm", "-f", name]).output().await;

        let output = tokio::process::Command::new("podman")
            .args([
                "run",
                "-d",
                "--name",
                name,
                "--network",
                "none",
                "--memory",
                "512m",
                "-v",
                &format!("{}:/workspace:z", workspace),
                image,
                "sleep",
                "infinity",
            ])
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// execute_isolated: Asynchronous Zircon-style IPC stream
    pub async fn execute_isolated(directive: &str, workspace_path: &str) -> Result<String> {
        println!(">> [SHIELD] Opening Async IPC Channel to Bio-Dome...");

        let mut child = Command::new("podman")
            .args([
                "run",
                "--rm",
                "--network",
                "none", // Air-gapped
                "--memory",
                "512m", // Resource governed
                "-v",
                &format!("{}:/workspace:z", workspace_path), // 'z' for Crostini/Termux safety
                "-w",
                "/workspace",
                "docker.io/library/rust:alpine",
                "sh",
                "-c",
                directive,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("Failed to capture Bio-Dome stdout");
        let mut reader = BufReader::new(stdout).lines();
        let mut output_buffer = String::new();

        // Real-time telemetry feed
        while let Ok(Some(line)) = reader.next_line().await {
            // Echoing the stream live to the host terminal
            println!(">> [BIO-DOME TELEMETRY]: {}", line);

            // Zircon-style Signal Check: The Arbiter's kill switch
            if line.contains("MALICIOUS_INTENT") {
                println!("!! [ARBITER] Rogue behavior detected. Severing IPC.");
                let _ = child.kill().await;
                return Err(anyhow::anyhow!("SHIELD_BREACH: Malicious intent intercepted."));
            }

            output_buffer.push_str(&line);
            output_buffer.push('\n');
        }

        let status = child.wait().await?;
        if status.success() {
            println!(">> [SHIELD] Bio-Dome process completed successfully.");
            Ok(output_buffer)
        } else {
            Err(anyhow::anyhow!("SHIELD_FAULT: Bio-Dome exited with {}", status))
        }
    }
}
