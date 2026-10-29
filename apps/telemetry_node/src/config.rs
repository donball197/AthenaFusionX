use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub endpoint: String,
    pub interval_ms: u64,
    pub enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:3000/api/vitals".to_string(),
            interval_ms: 3000,
            enabled: true,
        }
    }
}
