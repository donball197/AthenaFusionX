use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum InferenceStrategy {
    DeepReasoning,
    ShallowReasoning,
    ReactiveFlash,
    EmergencyEvac,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemVitals {
    pub cpu_temp_celsius: f32,
    pub free_ram_mb: u32,
    pub is_on_battery: bool,
}

pub struct DuetResourceGuard {
    pub last_temperature: AtomicU32,
}

impl DuetResourceGuard {
    pub fn new() -> Self {
        Self {
            last_temperature: AtomicU32::new(50.0f32.to_bits()),
        }
    }

    pub fn determine_strategy(&self, vitals: &SystemVitals) -> InferenceStrategy {
        if vitals.cpu_temp_celsius >= 79.0 || vitals.free_ram_mb < 300 {
            InferenceStrategy::EmergencyEvac
        } else if vitals.cpu_temp_celsius >= 75.0 || vitals.free_ram_mb < 450 {
            InferenceStrategy::ReactiveFlash
        } else if vitals.cpu_temp_celsius >= 68.0 {
            InferenceStrategy::ShallowReasoning
        } else {
            InferenceStrategy::DeepReasoning
        }
    }

    pub fn calculate_backoff_ms(&self, strategy: &InferenceStrategy) -> u64 {
        match strategy {
            InferenceStrategy::ShallowReasoning => 1500,
            _ => 0,
        }
    }

    pub fn calculate_predictive_pacing(&self, current_temp: f32) -> u64 {
        let last_bits = self.last_temperature.swap(current_temp.to_bits(), Ordering::Relaxed);
        let last_temp = f32::from_bits(last_bits);
        let delta_t = current_temp - last_temp;
        if delta_t > 0.4 && current_temp > 65.0 {
            tracing::warn!(
                "[GOVERNOR] Predictive intercept active. Delta T: +{:.2}°C/sec. Enforcing defensive break.",
                delta_t
            );
            return 2000;
        }
        0
    }
}

// Fixed for clippy::new_without_default alignment
impl Default for DuetResourceGuard {
    fn default() -> Self {
        Self::new()
    }
}
