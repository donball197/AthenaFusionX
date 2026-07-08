pub const MODEL_FLASH_LITE: &str = "gemini-3.1-flash-lite";
pub const MODEL_31B: &str = "gemma-4-31b-it";

pub struct AgentFactory;

impl AgentFactory {
    pub fn new() -> Self {
        Self
    }

    pub fn get_model_for_turn(turn: usize, error_count: usize) -> &'static str {
        if error_count > 0 {
            return MODEL_31B;
        }
        match turn {
            0..=4 => MODEL_FLASH_LITE,
            _ => MODEL_31B,
        }
    }

    pub fn failover_model(_current_model: &str) -> &'static str {
        MODEL_31B
    }
}

impl Default for AgentFactory {
    fn default() -> Self {
        Self::new()
    }
}
