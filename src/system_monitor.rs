use sysinfo::System;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct VitalsSnapshot {
    pub status: String,
    pub cpu_load: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
}

#[derive(Clone)]
pub struct SystemMonitor {
    sys: Arc<Mutex<System>>,
}

impl SystemMonitor {
    pub fn new(existing_sys: Arc<Mutex<System>>) -> Self {
        SystemMonitor { sys: existing_sys }
    }

    pub fn get_vitals(&self) -> VitalsSnapshot {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_cpu();
        sys.refresh_memory();

        let cpus = sys.cpus();
        let count = cpus.len() as f32;
        let total_usage: f32 = cpus.iter().map(|c| c.cpu_usage()).sum();
        let cpu_load = if count > 0.0 { total_usage / count } else { 0.0 };

        let ram_used = sys.used_memory() as f32 / 1_073_741_824.0;
        let ram_total = sys.total_memory() as f32 / 1_073_741_824.0;

        VitalsSnapshot {
            status: "ONLINE".into(),
            cpu_load,
            ram_used_gb: ram_used,
            ram_total_gb: ram_total,
        }
    }

    pub fn get_vital_signs_string(&self) -> String {
        let v = self.get_vitals();
        format!("CPU: {:.1}% | RAM: {:.2}GB / {:.2}GB", v.cpu_load, v.ram_used_gb, v.ram_total_gb)
    }
}
