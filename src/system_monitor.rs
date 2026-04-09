use crate::resonator::ResonatorCore;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Vitals {
    pub temperature: f32,
    pub cpu_load: f32,
    pub ram_gb: f32,
}

// SUTURE: Added Clone derivation to allow the struct to be shared across threads
#[derive(Clone, Debug)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self {
        SystemMonitor
    }

    pub fn get_vitals(&self) -> Vitals {
        // Link to the ResonatorCore fallback pulse
        let temperature = ResonatorCore::read_thermal_temp();
        
        // Stabilized mock metrics for Chromebook environment
        let cpu_load = 0.18; 
        let ram_gb = 1.4;

        Vitals {
            temperature,
            cpu_load,
            ram_gb,
        }
    }
}
