/**
 * Project Nexus Singularity - Smoke Test
 * Purpose: Pre-flight check for the v1.9 Kernel
 */

fn main() {
    // Unlocking the length constraint to prevent E0308
    let raw_telemetry: &[u8] =
        b"NODE_TEST | TEMP: 25.0C | BAT: 100% | CPU_LOAD: 0.10 | STATUS: SMOKE_TESTING";

    println!("\x1b[1;33m>> [SMOKE_TEST] Pulse Initialized...\x1b[0m");
    println!(">> Raw Data: {:?}", raw_telemetry);

    match std::str::from_utf8(raw_telemetry) {
        Ok(v) => println!("\x1b[1;36m>> [TELEMETRY]: {}\x1b[0m", v),
        Err(e) => eprintln!(">> [ERROR] Telemetry Corruption: {}", e),
    }
}
