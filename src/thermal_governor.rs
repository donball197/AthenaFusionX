use std::fs;
use tokio::time::{sleep, Duration};

pub struct ThermalGovernor {
    path: String,
}

impl ThermalGovernor {
    pub fn new() -> Self {
        Self { path: "/sys/class/thermal/thermal_zone0/temp".to_string() }
    }

    pub fn read_temp(&self) -> f32 {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .map(|t| t / 1000.0)
            .unwrap_or(0.0)
    }

    /// ⚡ Suture for Self-Healing Engine logic
    pub async fn apply_cooldown(&self) {
        let temp = self.read_temp();
        if temp > 65.0 {
            // Added 'temp' argument to the println! macro below
            println!("\x1b[33m[GOVERNOR] High Heat detected ({:.1}°C). Initiating Cooldown Nap...\x1b[0m", temp);
            sleep(Duration::from_secs(5)).await;
        }
    }

    pub fn get_status(&self) -> String {
        let temp = self.read_temp();
        if temp > 75.0 {
            format!("\x1b[31mCRITICAL: {:.1}°C (THROTTLING)\x1b[0m", temp)
        } else if temp > 60.0 {
            format!("\x1b[33mWARM: {:.1}°C (MONITORING)\x1b[0m", temp)
        } else {
            format!("\x1b[32mCOOL: {:.1}°C\x1b[0m", temp)
        }
    }
}
