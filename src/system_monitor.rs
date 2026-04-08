use serde::Serialize;
use std::fs;

#[derive(Serialize, Clone, Debug)]
pub struct Vitals {
    pub cpu_load: f32,
    pub ram_gb: f32,
    pub temperature: f32,
    pub resonance: f32,
}

#[derive(Clone)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self { SystemMonitor }

    fn read_temp() -> f32 {
        // Try standard thermal zones
        for zone in 0..5 {
            let path = format!("/sys/class/thermal/thermal_zone{}/temp", zone);
            if let Ok(raw) = fs::read_to_string(path) {
                if let Ok(t) = raw.trim().parse::<f32>() {
                    if t > 0.0 { return t / 1000.0; }
                }
            }
        }
        // Try HWMON (Common fallback for Crostini/Virtualization)
        for h in 0..5 {
            let path = format!("/sys/class/hwmon/hwmon{}/temp1_input", h);
            if let Ok(raw) = fs::read_to_string(path) {
                if let Ok(t) = raw.trim().parse::<f32>() {
                    if t > 0.0 { return t / 1000.0; }
                }
            }
        }
        0.0
    }

    pub fn get_vitals(&self) -> Vitals {
        let temp = Self::read_temp();

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
            if total > 0.0 { (total - available) / 1024.0 / 1024.0 } else { 0.0 }
        } else { 0.0 };
        
        let cpu_load = if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
            let load = loadavg.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0.0);
            (load / 4.0) * 100.0
        } else { 0.0 };

        Vitals {
            cpu_load, 
            ram_gb, 
            temperature: temp,
            resonance: 0.98,
        }
    }
}
