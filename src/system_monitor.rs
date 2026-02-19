use std::fs;

#[derive(Clone)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self {
        SystemMonitor
    }

    pub fn get_vital_signs(&self) -> String {
        let load = fs::read_to_string("/proc/loadavg").unwrap_or_default();
        let mem = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        
        let load_parts: Vec<&str> = load.split_whitespace().collect();
        let cpu_stress = load_parts.get(0).unwrap_or(&"0.00");

        let mut total_mem = 0;
        let mut avail_mem = 0;
        
        for line in mem.lines() {
            if line.starts_with("MemTotal:") { total_mem = parse_kb(line); }
            else if line.starts_with("MemAvailable:") { avail_mem = parse_kb(line); }
        }

        let used_gb = (total_mem - avail_mem) as f32 / 1024.0 / 1024.0;
        let total_gb = total_mem as f32 / 1024.0 / 1024.0;
        
        format!("CPU Load: {} | RAM: {:.2}GB / {:.2}GB", cpu_stress, used_gb, total_gb)
    }
}

fn parse_kb(line: &str) -> u64 {
    line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
}
