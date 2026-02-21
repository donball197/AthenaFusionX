use std::process::Command;
use serde::Serialize;
use anyhow::Result;

#[derive(Serialize, Debug, Clone)]
pub struct Container { pub id: String, pub image: String, pub status: String, pub names: String }

pub struct PodmanEngine;

impl PodmanEngine {
    pub fn ensure_synapse() -> Result<()> {
        let check = Command::new("podman").args(&["ps", "-q", "-f", "name=nexus_synapse"]).output()?;
        if check.stdout.is_empty() {
            Command::new("podman").args(&["run", "-d", "--name", "nexus_synapse", "-p", "6379:6379", "--restart", "always", "redis:alpine"]).output()?;
        }
        Ok(())
    }

    pub fn list() -> Vec<Container> {
        let output = Command::new("podman").args(&["ps", "-a", "--format", "{{.ID}}|{{.Image}}|{{.Status}}|{{.Names}}"]).output();
        if let Ok(out) = output {
            let raw = String::from_utf8_lossy(&out.stdout);
            raw.lines().filter_map(|line| {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    Some(Container { id: parts[0].to_string(), image: parts[1].to_string(), status: parts[2].to_string(), names: parts[3].to_string() })
                } else { None }
            }).collect()
        } else { Vec::new() }
    }

    pub fn spawn_drone(drone_type: &str) -> Result<String> {
        let (image, name, ports) = match drone_type {
            "secops" => ("nexus_secops_full", "nexus_secops", None),
            "devops" => ("node:18-alpine", "nexus_devops", None),
            "industrial" => ("postgres:alpine", "nexus_industrial", Some("5432")),
            _ => return Err(anyhow::anyhow!("Unknown Drone Class")),
        };
        let _ = Command::new("podman").args(&["rm", "-f", name]).output();
        let mut cmd = Command::new("podman");
        cmd.arg("run").arg("-d").arg("--name").arg(name).arg("--network").arg("slirp4netns:allow_host_loopback=true");
        if let Some(p) = ports { cmd.arg("-p").arg(format!("{}:{}", p, p)); }
        cmd.arg(image).arg("sleep").arg("infinity");
        let res = cmd.output()?;
        Ok(String::from_utf8_lossy(&res.stdout).trim().to_string())
    }

    pub fn spawn_remote_drone(ip: &str, user: &str, drone_type: &str) -> Result<String> {
        let cmd_str = format!("ssh -o ConnectTimeout=5 {}@{} 'podman run -d --name nexus_{}_rem nexus_secops_full sleep infinity'", user, ip, drone_type);
        let output = Command::new("sh").arg("-c").arg(&cmd_str).output()?;
        if output.status.success() { Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()) }
        else { Err(anyhow::anyhow!("Remote Link Refused")) }
    }

    pub fn kill_all() -> Result<()> {
        for c in Self::list() { if c.names != "nexus_synapse" { let _ = Command::new("podman").args(&["stop", &c.id]).output(); } }
        Ok(())
    }

    pub fn deploy(image: &str, port: &str) -> Result<String> {
        let res = Command::new("podman").args(&["run", "-d", "-p", &format!("{}:{}", port, port), image]).output()?;
        Ok(String::from_utf8_lossy(&res.stdout).trim().to_string())
    }

    pub fn nuke(id: &str) -> Result<()> { let _ = Command::new("podman").args(&["rm", "-f", id]).output(); Ok(()) }
}
