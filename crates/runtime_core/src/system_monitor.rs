use sysinfo::{System, SystemExt, CpuExt};
use std::sync::Mutex;

pub struct SystemMonitor {
    sys: Mutex<System>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self { sys: Mutex::new(sys) }
    }

    pub fn get_vitals(&self) -> serde_json::Value {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_cpu();
        sys.refresh_memory();

        let cpu_usage = sys.global_cpu_info().cpu_usage();
        let total_mem = sys.total_memory() / 1048576; // MB
        let used_mem = sys.used_memory() / 1048576;   // MB

        // Attempt to read the ARM64 thermal zone directly
        let temp = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .unwrap_or_else(|_| "0".to_string())
            .trim()
            .parse::<f32>()
            .unwrap_or(0.0) / 1000.0;

        serde_json::json!({
            "cpu_usage": cpu_usage,
            "memory_used_mb": used_mem,
            "memory_total_mb": total_mem,
            "temperature_c": temp,
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        })
    }
}
