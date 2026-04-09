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
        // Broad search for ANY thermal metric in the virtualized tree
        let paths = [
            "/sys/class/thermal/thermal_zone0/temp",
            "/sys/class/thermal/thermal_zone1/temp",
            "/sys/class/hwmon/hwmon0/temp1_input",
            "/sys/class/hwmon/hwmon1/temp1_input",
            "/sys/class/hwmon/hwmon2/temp1_input",
            "/sys/devices/virtual/thermal/thermal_zone0/temp",
        ];

        for path in paths {
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
            let mut available = 0.0;
            for line in meminfo.lines() {
                if line.starts_with("MemAvailable:") {
                    available = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0.0);
                }
            }
            available / 1024.0 / 1024.0
        } else { 0.0 };
        
        let cpu_load = if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
            let load = loadavg.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0.0);
            (load / 4.0) * 100.0
        } else { 0.0 };

        Vitals {
            cpu_load, ram_gb, temperature: temp, resonance: (100.0 - cpu_load) / 100.0,
        }
    }
}
