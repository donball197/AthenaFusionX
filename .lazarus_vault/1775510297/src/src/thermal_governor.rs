use std::fs;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoolingState {
    Normal,
    Throttled,
    Critical,
}

pub struct ThermalGovernor {
    pub warn_threshold: f32,
    pub crit_threshold: f32,
}

impl Default for ThermalGovernor {
    fn default() -> Self {
        Self { warn_threshold: 68.0, crit_threshold: 78.0 }
    }
}

impl ThermalGovernor {
    pub fn new() -> Self { Self::default() }

    pub fn read_temp() -> f32 {
        fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .map(|t| t / 1000.0)
            .unwrap_or(0.0)
    }

    pub fn get_state(&self) -> CoolingState {
        let t = Self::read_temp();
        if t >= self.crit_threshold { CoolingState::Critical }
        else if t >= self.warn_threshold { CoolingState::Throttled }
        else { CoolingState::Normal }
    }

    pub async fn apply_cooldown(&self) {
        match self.get_state() {
            CoolingState::Critical => {
                println!("\x1b[31m>> [THERMAL] CRITICAL {}°C — aggressive cooldown\x1b[0m", Self::read_temp());
                sleep(Duration::from_secs(10)).await;
            }
            CoolingState::Throttled => {
                println!("\x1b[33m>> [THERMAL] WARN {}°C — light cooldown\x1b[0m", Self::read_temp());
                sleep(Duration::from_secs(4)).await;
            }
            _ => {}
        }
    }
}
