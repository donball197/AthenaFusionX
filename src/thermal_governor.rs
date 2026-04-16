/**
 * Copyright 2026 Donald Ball
 * Project Nexus Singularity - v1.9.6 (Synthetic Thermal Patch)
 */

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
    pub fn new() -> Self {
        Self::default()
    }

    // ⚡ SUTURE: Synthetic Fallback Logic
    pub fn read_temp() -> f32 {
        // 1. Try to read physical hardware (works on Motorola/Termux)
        for zone in 0..5 {
            let path = format!("/sys/class/thermal/thermal_zone{}/temp", zone);
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(t) = raw.trim().parse::<f32>() {
                    if t > 0.0 {
                        return t / 1000.0;
                    }
                }
            }
        }
        
        // 2. Fallback: Synthetic calculation based on CPU load (works on Crostini)
        if let Ok(load_data) = fs::read_to_string("/proc/loadavg") {
            if let Some(one_min_load_str) = load_data.split_whitespace().next() {
                if let Ok(load) = one_min_load_str.parse::<f32>() {
                    // Base temp 42C + (Load * 12C). Caps at 95C.
                    let synthetic_temp = 42.0 + (load * 12.0);
                    return synthetic_temp.min(95.0);
                }
            }
        }
        
        // 3. Absolute failsafe
        40.0 
    }

    pub fn get_state(&self) -> CoolingState {
        let t = Self::read_temp();
        if t >= self.crit_threshold {
            CoolingState::Critical
        } else if t >= self.warn_threshold {
            CoolingState::Throttled
        } else {
            CoolingState::Normal
        }
    }

    pub async fn apply_cooldown(&self) {
        match self.get_state() {
            CoolingState::Critical => {
                println!(
                    "\x1b[31m>> [THERMAL] CRITICAL {:.1}°C — Aggressive cooldown\x1b[0m",
                    Self::read_temp()
                );
                sleep(Duration::from_secs(12)).await;
            }
            CoolingState::Throttled => {
                println!(
                    "\x1b[33m>> [THERMAL] WARN {:.1}°C — Light cooldown\x1b[0m",
                    Self::read_temp()
                );
                sleep(Duration::from_secs(5)).await;
            }
            _ => {}
        }
    }
}
