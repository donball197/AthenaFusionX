/**
 * Project Nexus Singularity v1.9.7 - Hardened Resonator
 * Frequency: 528Hz Pulse Logic | Substrate-Linked
 */
use crate::thermal_governor::ThermalGovernor;

pub struct ResonatorCore {
    pub frequency: u32,
    governor: ThermalGovernor,
}

impl ResonatorCore {
    pub fn new() -> Self {
        Self { frequency: 528, governor: ThermalGovernor::new() }
    }

    pub fn sync(&self) {
        let status = self.governor.get_status();
        println!(
            "\x1b[1;34m>> [RESONATOR] Frequency Locked: {}Hz | Thermal: {}\x1b[0m",
            self.frequency, status
        );
    }

    /// Optimized thermal read utilizing the Governor's pooled logic
    pub fn read_thermal_temp(&self) -> f32 {
        self.governor.read_temp()
    }
}

pub type Resonator = ResonatorCore;
