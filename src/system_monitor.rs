use sysinfo::{System, Networks};
use std::sync::{Arc, Mutex};

pub struct VitalsSnapshot {
    pub status: String,
    pub cpu_load: f32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
}

pub struct SystemMonitor {
    sys: Arc<Mutex<System>>,
    nets: Arc<Mutex<Networks>>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let nets = Networks::new_with_refreshed_list();
        
        SystemMonitor { 
            sys: Arc::new(Mutex::new(sys)),
            nets: Arc::new(Mutex::new(nets)),
        }
    }

    pub fn get_vitals(&self) -> VitalsSnapshot {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_load = sys.global_cpu_usage(); 
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
