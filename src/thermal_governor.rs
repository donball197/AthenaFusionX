// src/thermal_governor.rs - ARM64 Thermal Awareness
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

    fn read_temp() -> f32 {
        for zone in 0..5 {
            let path = format!("/sys/class/thermal/thermal_zone{}/temp", zone);
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(t) = raw.trim().parse::<f32>() {
                    if t > 0.0 { return t / 1000.0; }
                }
            }
        }
        0.0
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
                println!("\x1b[31m>> [THERMAL] CRITICAL {}°C — Aggressive cooldown\x1b[0m", Self::read_temp());
                sleep(Duration::from_secs(12)).await;
            }
            CoolingState::Throttled => {
                println!("\x1b[33m>> [THERMAL] WARN {}°C — Light cooldown\x1b[0m", Self::read_temp());
                sleep(Duration::from_secs(5)).await;
            }
            _ => {}
        }
    }
}
