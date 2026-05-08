use std::time::Duration;
use tokio::time::sleep;
use tracing::{warn, info};

pub struct ThermalGovernor;

impl ThermalGovernor {
    const CRITICAL_TEMP: f32 = 65.0; // Lenovo Duet throttles heavily at 70C

    pub async fn enforce_cooldown() {
        let temp_str = tokio::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").await.unwrap_or_else(|_| "0".to_string());
        let temp_c = temp_str.trim().parse::<f32>().unwrap_or(0.0) / 1000.0;

        if temp_c >= Self::CRITICAL_TEMP {
            warn!("\x1b[31m[THERMAL GOVERNOR] CRITICAL HEAT DETECTED: {:.1}°C. Enforcing 15-second DAG pause.\x1b[0m", temp_c);
            let _ = crate::metrics::METRICS.inc_failure(); // Log a thermal strain as a system fault
            sleep(Duration::from_secs(15)).await;
            info!("\x1b[32m[THERMAL GOVERNOR] Cooldown complete. Resuming operations.\x1b[0m");
        }
    }
}
