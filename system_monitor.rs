pub struct Vitals {
    pub cpu_load: f32,
    pub ram_gb: f32,
    pub total_ram_gb: f32,
    pub temperature: f32,
}
pub struct SystemMonitor {}
impl SystemMonitor {
    pub fn new() -> Self { Self {} }
    pub fn get_vitals(&mut self) -> Vitals {
        Vitals { cpu_load: 0.0, ram_gb: 0.0, total_ram_gb: 0.0, temperature: 0.0 }
    }
}
