use std::process::Command;
use std::env;
use serde::Serialize;
use anyhow::Result;
use std::fs;

#[derive(Serialize, Debug, Clone)]
pub struct Container { pub id: String, pub image: String, pub status: String, pub names: String }

pub struct PodmanEngine;

impl PodmanEngine {
    fn get_resource_limits() -> (String, String) {
        let mem = env::var("NEXUS_DRONE_MEM").unwrap_or_else(|_| "128m".to_string());
        let cpu = env::var("NEXUS_DRONE_CPU").unwrap_or_else(|_| "0.5".to_string());
        (mem, cpu)
    }

    pub fn ensure_synapse() -> Result<()> {
        let check = Command::new("podman").args(&["ps", "-q", "-f", "name=nexus_synapse"]).output()?;
        if check.stdout.is_empty() {
            Command::new("podman").args(&["run", "-d", "--name", "nexus_synapse", "--memory=64m", "-p", "6379:6379", "--restart", "always", "redis:alpine"]).output()?;
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
        let (mem_limit, cpu_limit) = Self::get_resource_limits();
        let custom_name_string; 
        
        let (image, name, ports) = match drone_type {
            "secops" => ("alpine:latest", "nexus_secops", None),
            "devops" => ("node:18-alpine", "nexus_devops", None),
            "industrial" => ("postgres:alpine", "nexus_industrial", Some("5432")),
            custom => {
                custom_name_string = format!("nexus_{}", custom);
                ("alpine:latest", custom_name_string.as_str(), None)
            }
        };
        
        let _ = Command::new("podman").args(&["rm", "-f", name]).output();
        
        let mut cmd = Command::new("podman");
        cmd.arg("run").arg("-d").arg("--name").arg(name).arg("--init")
           .arg(format!("--memory={}", mem_limit)).arg(format!("--cpus={}", cpu_limit))
           .arg("--network").arg("slirp4netns:allow_host_loopback=true");

        if drone_type == "secops" {
            if let Ok(cwd) = std::env::current_dir() {
                cmd.arg("-v").arg(format!("{}/targets:/workspace", cwd.display()));
            }
            cmd.arg(image).arg("sh").arg("-c").arg("apk add --no-cache nmap curl bind-tools && sleep infinity");
        } else {
            cmd.arg(image).arg("sleep").arg("infinity");
        }

        if let Some(p) = ports { cmd.arg("-p").arg(format!("{}:{}", p, p)); }
        
        let res = cmd.output()?;
        Ok(String::from_utf8_lossy(&res.stdout).trim().to_string())
    }

    pub fn forge_drone(name: &str, dockerfile: &str) -> Result<String> {
        let build_dir = format!("hives/{}", name);
        fs::create_dir_all(&build_dir)?;
        fs::write(format!("{}/Dockerfile", build_dir), dockerfile)?;
        
        let _ = Command::new("podman").args(&["image", "prune", "-f"]).output();
        
        let output = Command::new("podman").args(&["build", "-t", name, &build_dir]).output()?;
        if output.status.success() { Ok(format!("Forged {}", name)) } else { Err(anyhow::anyhow!("Forge Failed")) }
    }

    pub fn kill_all() -> Result<()> {
        for c in Self::list() { 
            if c.names != "nexus_synapse" { 
                let _ = Command::new("podman").args(&["stop", &c.id]).output(); 
            } 
        }
        Ok(())
    }

    pub fn deploy(image: &str, port: &str) -> Result<String> {
        let res = Command::new("podman").args(&["run", "-d", "--memory=256m", "-p", &format!("{}:{}", port, port), image]).output()?;
        Ok(String::from_utf8_lossy(&res.stdout).trim().to_string())
    }
    
    pub fn nuke(id: &str) -> Result<()> { let _ = Command::new("podman").args(&["rm", "-f", id]).output(); Ok(()) }
}
