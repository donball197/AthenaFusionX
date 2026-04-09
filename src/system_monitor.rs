use serde::Serialize;
use crate::resonator::ResonatorCore;
use std::fs;

#[derive(Serialize, Clone, Debug)]
pub struct Vitals {
    pub cpu_load: f32,
    pub ram_gb: f32,
    pub temperature: f32,
    pub resonance: f32,
}

#[derive(Clone, Default)]
pub struct SystemMonitor;

impl SystemMonitor {
    pub fn new() -> Self { SystemMonitor }

    pub fn get_vitals(&self) -> Vitals {
        // High-fidelity thermal data from the Zig Reflex
        let temp = ResonatorCore::read_thermal_temp();

        let ram_gb = if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut available = 0.0;
            for line in meminfo.lines() {
                if line.starts_with("MemAvailable:") {
                    available = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0.0);
                }
            }
            available / 1024.0 / 1024.0
        } else { 0.0 };
        
        let cpu_load = if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
            let load = loadavg.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0.0);
            (load / 4.0) * 100.0
        } else { 0.0 };

        Vitals {
            cpu_load,
            ram_gb,
            temperature: temp,
            resonance: (100.0 - cpu_load) / 100.0,
        }
    }
}
