/**
 * Project Nexus Singularity - Thermal Probe v1.9.7
 * Diagnostic binary for ARM64 substrate
 */
use nexus_singularity::resonator::ResonatorCore;

fn main() {
    // 1. Instantiate the Resonator (which now holds the ThermalGovernor)
    let core = ResonatorCore::new();

    // 2. Pulse the hardware via the instance
    let temp = core.read_thermal_temp();

    // 3. Raw output for shell script consumption
    println!("{:.1}", temp);
}
