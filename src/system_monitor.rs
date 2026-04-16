use sysinfo::{System, Components};

pub struct Vitals {
    pub ram_gb: f64,
    pub total_ram_gb: f64,
    pub temperature: f32,
    pub cpu_load: f32,
}

pub struct SystemMonitor {
    sys: System,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self { sys }
    }

    pub fn get_vitals(&mut self) -> Vitals {
        // Refresh system state
        self.sys.refresh_memory();
        self.sys.refresh_cpu_all();

        let total_mem = self.sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_mem = self.sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        
        // v0.31+ Thermal API: Components is now a separate refreshed list
        let components = Components::new_with_refreshed_list();
        let temp = components.iter()
            .find(|c| c.label().contains("CPU") || c.label().contains("Package"))
            .map(|c| c.temperature().unwrap_or(0.0))
            .unwrap_or(0.0);

        let load = self.sys.global_cpu_usage() / 100.0;

        Vitals {
            ram_gb: used_mem,
            total_ram_gb: total_mem,
            temperature: temp,
            cpu_load: load,
        }
    }
}
