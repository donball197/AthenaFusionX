use sysinfo::System;

pub struct IndustrialDrone;

impl IndustrialDrone {
    pub async fn auto_throttle() -> Result<(), String> {
        // Lean memory check
        let mut sys = System::new();
        sys.refresh_memory();

        let mem_usage = sys.used_memory() as f32 / sys.total_memory() as f32;

        if mem_usage > 0.80 {
            println!(">> [INDUSTRIAL] RAM THRESHOLD BREACHED: {:.1}%. THROTTLING.", mem_usage * 100.0);
            // The active counter-measure
            let _ = std::process::Command::new("podman").args(&["restart", "nexus_industrial"]).output();
        }

        Ok(())
    }
}
