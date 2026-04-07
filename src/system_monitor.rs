use serde::Serialize;
use std::fs;

#[derive(Serialize, Clone, Debug)]
pub struct Vitals {
    pub cpu_load: f32,
    pub ram_gb: f32,
    pub temperature: f32,
    pub resonance: f32,
}

// UPGRADE: Added Clone derivation to allow Arc/Thread sharing
#[derive(Clone)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self { SystemMonitor }

    pub fn get_vitals(&self) -> Vitals {
        let temp = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .map(|t| t / 1000.0)
            .unwrap_or(0.0);

        let ram_gb = if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut total = 0.0;
            let mut available = 0.0;
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    total = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0.0);
                } else if line.starts_with("MemAvailable:") {
                    available = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0.0);
                }
            }
            if total > 0.0 {
                (total - available) / 1024.0 / 1024.0
            } else {
                0.85
            }
        } else {
            0.85
        };
        
        let cpu_load = if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
            loadavg.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0.15)
        } else {
            0.15
        };

        Vitals {
            cpu_load, 
            ram_gb, 
            temperature: temp,
            resonance: 0.98,
        }
    }
}
