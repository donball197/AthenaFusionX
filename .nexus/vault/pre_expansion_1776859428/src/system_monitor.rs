use sysinfo::System;

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
        self.sys.refresh_memory();
        self.sys.refresh_cpu_all();

        let total_mem = self.sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_mem = self.sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let load = self.sys.global_cpu_usage() / 100.0;

        // ⚡ PUMP: Push real CPU load into the Zig kernel for synthetic thermal model
        unsafe {
            crate::kernel::ffi::set_substrate_load(load);
        }

        // Synthetic temperature: 30°C idle → 70°C full load
        let temp = (load * 40.0) + 30.0;

        Vitals { ram_gb: used_mem, total_ram_gb: total_mem, temperature: temp, cpu_load: load }
    }
}
