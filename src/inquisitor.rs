use std::process::Command;
use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use chrono::Local;
use std::fs;

pub struct Inquisitor;

impl Inquisitor {
    pub async fn native_port_scan(target: &str) -> Result<String> {
        if target.contains(';') || target.contains('&') || target.contains('|') {
            return Err(anyhow::anyhow!("SECURITY OVERRIDE: Invalid target syntax."));
        }

        let tactical_ports = vec![21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 443, 445, 993, 995, 1723, 3306, 3389, 5900, 8080, 3000, 5000];
        let mut open_ports = Vec::new();

        for port in tactical_ports {
            let addr = format!("{}:{}", target, port);
            let conn = TcpStream::connect(&addr);
            if let Ok(Ok(_)) = timeout(Duration::from_millis(500), conn).await {
                open_ports.push(port.to_string());
            }
        }

        let result = if open_ports.is_empty() {
            format!(">> [RECON] Target {} scanned. No tactical ports open.", target)
        } else {
            format!(">> [RECON] Target {} has open ports: [{}]", target, open_ports.join(", "))
        };

        Self::log_to_vault("nmap", target, &result);
        Ok(result)
    }

    pub fn execute_tactical(cmd: &str, target: &str) -> Result<String> {
        if target.contains(';') || target.contains('&') || target.contains('|') {
            return Err(anyhow::anyhow!("SECURITY OVERRIDE: Invalid target syntax."));
        }

        let drone_path = format!("/workspace/{}", target);
        let mut args = vec!["exec", "nexus_secops"];

        match cmd {
            "ls" => args.extend_from_slice(&["ls", "-lh", "/workspace"]),
            "radare2" => args.extend_from_slice(&["r2", "-c", "aaa; afl; q", &drone_path]),
            "strings" => args.extend_from_slice(&["strings", &drone_path]),
            "file" => args.extend_from_slice(&["file", &drone_path]),
            "nuclei" => args.extend_from_slice(&["nuclei", "-u", target, "-t", "/workspace/nexus-recon.yaml", "-no-color"]),
            _ => return Err(anyhow::anyhow!("Unknown tactical command: {}", cmd)),
        }

        let output = Command::new("podman")
            .args(&args)
            .output()
            .context("Failed to communicate with SecOps drone")?;

        if output.status.success() {
            let result_str = String::from_utf8_lossy(&output.stdout).to_string();
            Self::log_to_vault(cmd, target, &result_str);
            Ok(result_str)
        } else {
            let error = String::from_utf8_lossy(&output.stderr).to_string();
            Err(anyhow::anyhow!("SecOps Drone Error: {}", error))
        }
    }

    fn log_to_vault(cmd: &str, target: &str, output: &str) {
        let _ = fs::create_dir_all("vault");
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        // Clean the target string so it doesn't break filenames (e.g., replace https:// with nothing)
        let safe_target = target.replace("https://", "").replace("http://", "").replace("/", "_");
        let filename = format!("vault/strike_{}_{}_{}.md", cmd, safe_target, timestamp);
        
        let log_content = format!(
            "# Tactical Strike Report\n**Command:** {}\n**Target:** {}\n**Timestamp:** {}\n\n## Output\n```text\n{}\n```\n", 
            cmd, target, timestamp, output
        );
        
        let _ = fs::write(filename, log_content);
    }
}
