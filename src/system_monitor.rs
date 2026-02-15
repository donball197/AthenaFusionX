use std::fs;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub ram_usage: f32,
    pub ram_total: u64,
    pub ram_used: u64,
}

#[derive(Clone)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self {
        SystemMonitor
    }

    pub fn get_stats(&self) -> SystemStats {
        let (cpu, _) = self.calculate_cpu();
        let (used, total, percent) = self.get_memory();
        
        SystemStats {
            cpu_usage: cpu,
            ram_usage: percent,
            ram_total: total,
            ram_used: used,
        }
    }

    fn calculate_cpu(&self) -> (f32, u64) {
        // Simple /proc/stat parser
        if let Ok(content) = fs::read_to_string("/proc/stat") {
            if let Some(line) = content.lines().next() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 8 {
                    let user: f32 = parts[1].parse().unwrap_or(0.0);
                    let idle: f32 = parts[4].parse().unwrap_or(0.0);
                    // Extremely basic load calculation for demo stability
                    // A real implementation requires diffing against previous tick
                    return ((user / (user + idle)) * 100.0, 0); 
                }
            }
        }
        (0.0, 0)
    }

    fn get_memory(&self) -> (u64, u64, f32) {
        let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let mut total = 0;
        let mut available = 0;
        
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) * 1024;
            } else if line.starts_with("MemAvailable:") {
                available = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) * 1024;
            }
        }
        
        if total == 0 { return (0, 0, 0.0); }
        let used = total.saturating_sub(available);
        let percent = (used as f32 / total as f32) * 100.0;
        
        (used, total, percent)
    }
}
