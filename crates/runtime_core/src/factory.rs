use crate::model_config::{get_light_models, get_heavy_models};

pub struct AgentFactory;

impl AgentFactory {
    pub fn new() -> Self { Self }

    pub fn get_model_for_turn(turn: usize, error_count: usize) -> String {
        if error_count > 0 {
            return get_heavy_models().first().cloned().unwrap_or_else(|| "gemini-3.1-flash-lite".to_string());
        }
        match turn {
            0..=4 => get_light_models().first().cloned().unwrap_or_else(|| "gemini-3.1-flash-lite".to_string()),
            _     => get_heavy_models().first().cloned().unwrap_or_else(|| "gemini-3.1-flash-lite".to_string()),
        }
    }

    pub fn failover_model(current_model: &str) -> String {
        let heavy = get_heavy_models();
        let light = get_light_models();
        
        if let Some(pos) = heavy.iter().position(|m| m == current_model) {
            if pos + 1 < heavy.len() { return heavy[pos+1].clone(); }
        }
        if let Some(pos) = light.iter().position(|m| m == current_model) {
            if pos + 1 < light.len() { return light[pos+1].clone(); }
        }
        
        // Ultimate fallback
        "gemini-3.1-flash-lite".to_string()
    }
}

impl Default for AgentFactory {
    fn default() -> Self { Self::new() }
}
