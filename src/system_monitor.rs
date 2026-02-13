use serde::Serialize;
use std::fs;

#[derive(Serialize, Clone)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub ram_usage: f32,
    pub temperature: f32,
}

pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self {
        SystemMonitor
    }

    pub fn get_temp(&self) -> f32 {
        // Standard Android thermal zone path
        let temp_str = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .unwrap_or_else(|_| "0".into());
        
        let temp_raw: f32 = temp_str.trim().parse().unwrap_or(0.0);
        
        // Android temps are usually in millidegrees (e.g. 42000 for 42C)
        if temp_raw > 1000.0 { temp_raw / 1000.0 } else { temp_raw }
    }

    pub fn get_stats(&self) -> SystemStats {
        // Mocking CPU/RAM for the dashboard pulse
        SystemStats {
            cpu_usage: 15.5, // We can integrate 'sysinfo' crate later for real %
            ram_usage: 65.0,
            temperature: self.get_temp(),
        }
    }
}
