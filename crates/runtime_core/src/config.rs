use std::env;

#[derive(Clone, Debug)]
pub struct NexusConfig {
    pub heavy_model: String,
    pub light_model: String,
    pub light_cascade: Vec<String>,
    pub heavy_cascade: Vec<String>,
}

impl NexusConfig {
    pub fn from_env() -> Self {
        let light_cascade_str = env::var("NEXUS_LIGHT_CASCADE").unwrap_or_default();
        let heavy_cascade_str = env::var("NEXUS_HEAVY_CASCADE").unwrap_or_default();

        let light_cascade = light_cascade_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let heavy_cascade = heavy_cascade_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            heavy_model: env::var("NEXUS_HEAVY_MODEL")
                .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".to_string()),
            light_model: env::var("NEXUS_LIGHT_MODEL")
                .unwrap_or_else(|_| "gemini-3.5-flash-lite".to_string()),
            light_cascade,
            heavy_cascade,
        }
    }
}
