/**
 * Project Nexus Singularity v1.9 - Resonator Core
 * Frequency: 528Hz Pulse Logic
 */
use std::fs;

pub struct ResonatorCore {
    pub frequency: u32,
}

impl ResonatorCore {
    pub fn new() -> Self {
        Self { frequency: 528 }
    }

    pub fn sync(&self) {
        println!("\x1b[1;34m>> [RESONATOR] Frequency Locked: {}Hz\x1b[0m", self.frequency);
    }

    // Thermal read logic mapped for ARM64 Linux / Crostini
    pub fn read_thermal_temp() -> f32 {
        match fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
            Ok(val) => {
                let raw_temp: f32 = val.trim().parse().unwrap_or(45000.0);
                raw_temp / 1000.0 // Convert millidegrees to standard Celsius
            },
            Err(_) => {
                // Fail-safe if the container prison locks down hardware access
                45.5
            }
        }
    }
}

// Alias for universal compatibility across the crate
pub type Resonator = ResonatorCore;
