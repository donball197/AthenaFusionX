use sysinfo::System;
use std::sync::{Arc, Mutex};

#[derive(Clone, serde::Serialize)]
pub struct VitalsSnapshot {
    pub status: String,
    pub cpu_usage: f32,    // Matches Legacy UI
    pub cpu_load: f32,     // Matches Dev Logic
    pub ram_usage: String, // Matches Legacy UI "1.2GB / 4.0GB"
    pub ram_used_gb: f32,  // Matches Dev Logic
    pub resonance: f32,
    pub temperature: f32,
}

#[derive(Clone)]
pub struct SystemMonitor {
    sys: Arc<Mutex<System>>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        SystemMonitor { sys: Arc::new(Mutex::new(System::new_all())) }
    }

    pub fn get_vitals(&self) -> VitalsSnapshot {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_all();
        let used = sys.used_memory() as f32 / 1024.0 / 1024.0 / 1024.0;
        let total = sys.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0;
        let cpu = sys.global_cpu_info().cpu_usage();

        VitalsSnapshot {
            status: if used > (total * 0.9) { "CRITICAL" } else { "ONLINE" }.into(),
            cpu_usage: cpu,
            cpu_load: cpu,
            ram_usage: format!("{:.2}GB / {:.1}GB", used, total),
            ram_used_gb: used,
            resonance: 0.98,
            temperature: crate::thermal_governor::ThermalGovernor::read_temp(),
        }
    }

    pub fn get_vital_signs_string(&self) -> String {
        let v = self.get_vitals();
        format!("CPU: {:.1}% | RAM: {}", v.cpu_usage, v.ram_usage)
    }
}
impl Default for SystemMonitor { fn default() -> Self { Self::new() } }
