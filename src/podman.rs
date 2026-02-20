use std::process::Command;
use serde::Serialize;
use anyhow::Result;

#[derive(Serialize, Debug)]
pub struct Container {
    pub id: String,
    pub image: String,
    pub status: String,
    pub names: String,
}

pub struct PodmanEngine;

impl PodmanEngine {
    pub fn list() -> Vec<Container> {
        let output = Command::new("podman")
            .args(&["ps", "-a", "--format", "{{.ID}}|{{.Image}}|{{.Status}}|{{.Names}}"])
            .output();

        if let Ok(out) = output {
            let raw = String::from_utf8_lossy(&out.stdout);
            raw.lines().filter_map(|line| {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    Some(Container {
                        id: parts[0].to_string(),
                        image: parts[1].to_string(),
                        status: parts[2].to_string(),
                        names: parts[3].to_string(),
                    })
                } else { None }
            }).collect()
        } else {
            Vec::new()
        }
    }

    pub fn deploy(image: &str, port: &str) -> Result<String> {
        let res = Command::new("podman")
            .args(&["run", "-d", "-p", &format!("{}:{}", port, port), image])
            .output()?;
        Ok(String::from_utf8_lossy(&res.stdout).trim().to_string())
    }

    pub fn nuke(id: &str) -> Result<()> {
        Command::new("podman").args(&["rm", "-f", id]).output()?;
        Ok(())
    }
}
