use crate::resonator; // SUTURE: Changed from 'mod' to 'use crate' to resolve E0583

pub fn execute() {
    println!(">> [NEXUS] INITIATING HIVE PULSE CHECK...");
    
    // Zig NPU pulse via Resonator Core
    let _ = resonator::ResonatorCore::ignite_vault("vault/pulse");
    
    let thermal = resonator::ResonatorCore::read_thermal_temp();
    println!(">> [STATUS] THERMAL RESONANCE: {:.1}°C", thermal);
    
    if thermal > 0.0 {
        println!(">> [SUCCESS] THE HIVE IS ALIVE.");
    } else {
        println!(">> [WARNING] FRAGMENTATION DETECTED IN SENSORY STRANDS.");
    }
}
