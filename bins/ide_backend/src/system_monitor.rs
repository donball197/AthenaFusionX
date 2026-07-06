use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

pub struct Vitals {
    pub cpu_load: f32,
    pub ram_gb: f32,
    pub total_ram_gb: f32,
    pub temperature: f32,
}

pub struct SystemMonitor {
    sys: System,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_all();
        Self { sys }
    }

    pub fn get_vitals(&mut self) -> Vitals {
        // Updated for modern sysinfo API (no extension traits required)
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();

        // Direct method call replacement
        let cpu_load = self.sys.global_cpu_usage() / 100.0;

        // Convert raw bytes to Gigabytes
        let ram_gb   = (self.sys.used_memory() as f32) / 1_073_741_824.0;
        let total_ram_gb = (self.sys.total_memory() as f32) / 1_073_741_824.0;

        // Poll the primary Linux thermal zone file descriptor for core ARM64 temperature
        let temperature = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .unwrap_or_else(|_| "45000".to_string())
            .trim()
            .parse::<f32>()
            .unwrap_or(45000.0) / 1000.0;

        Vitals {
            cpu_load,
            ram_gb,
            total_ram_gb,
            temperature,
        }
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}
