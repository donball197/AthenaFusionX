use std::fs;

#[derive(Clone, Default)]
pub struct SystemSnapshot {
    pub cpu_load: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub status: String,
}

pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self { Self }

    pub fn get_vitals(&self) -> SystemSnapshot {
        let load = fs::read_to_string("/proc/loadavg").unwrap_or_default();
        let mem = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        
        let cpu_load = load.split_whitespace().next()
            .and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);

        let mut total_kb = 0;
        let mut avail_kb = 0;
        for line in mem.lines() {
            if line.starts_with("MemTotal:") { total_kb = parse_kb(line); }
            else if line.starts_with("MemAvailable:") { avail_kb = parse_kb(line); }
        }

        let total_gb = total_kb as f32 / 1024.0 / 1024.0;
        let used_gb = (total_kb - avail_kb) as f32 / 1024.0 / 1024.0;

        SystemSnapshot {
            cpu_load,
            ram_used_gb: used_gb,
            ram_total_gb: total_gb,
            status: if cpu_load > 4.0 { "THROTTLED".into() } else { "ONLINE".into() },
        }
    }

    pub fn get_vital_signs_string(&self) -> String {
        let s = self.get_vitals();
        format!("CPU: {:.2} | RAM: {:.2}GB/{:.2}GB", s.cpu_load, s.ram_used_gb, s.ram_total_gb)
    }
}

fn parse_kb(line: &str) -> u64 {
    line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
}
