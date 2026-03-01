use crate::resonator::ResonatorCore;
use anyhow::Result;

pub fn run_vision_sentry() -> String {
    let energy = ResonatorCore::scan_local_energy("").unwrap_or(0.0);
    if energy > 0.9 {
        ">> [NEXUS] VISION: ZIG RESONATOR ACTIVE (528Hz)".into()
    } else {
        ">> [NEXUS] VISION: LOW POTENTIAL".into()
    }
}

pub fn run_local_reflex() -> String {
    ">> [NEXUS] REFLEX: HARDWARE-LINKED".into()
}

pub fn ignite_agent(path: &str) -> Result<()> {
    ResonatorCore::ignite_vault(path).map(|_| ())
}
