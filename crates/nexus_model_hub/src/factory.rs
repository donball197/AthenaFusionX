use std::env;

pub fn get_flash_lite_model() -> String {
    env::var("NEXUS_LIGHT_MODEL").unwrap_or_else(|_| "gemini-3.5-flash-lite".to_string())
}

pub fn get_heavy_model() -> String {
    env::var("NEXUS_HEAVY_MODEL")
        .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".to_string())
}

pub struct AgentFactory;

impl AgentFactory {
    pub fn new() -> Self {
        Self
    }

    pub fn get_model_for_turn(turn: usize, error_count: usize) -> String {
        if error_count > 0 {
            return get_heavy_model();
        }
        match turn {
            0..=4 => get_flash_lite_model(),
            _ => get_heavy_model(),
        }
    }

    pub fn failover_model(_current_model: &str) -> String {
        get_heavy_model()
    }
}

impl Default for AgentFactory {
    fn default() -> Self {
        Self::new()
    }
}
