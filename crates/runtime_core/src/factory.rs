pub const MODEL_FLASH_LITE: &str = "gemini-3.1-flash-lite";
pub const MODEL_31B: &str = "gemma-4-31b-it";
pub const MODEL_26B_MOE: &str = "gemma-4-26b-a4b-it";

pub struct AgentFactory;

impl AgentFactory {
    /// Constructs a stateless factory with no fields.
    pub fn new() -> Self { Self }

    pub fn get_model_for_turn(turn: usize, error_count: usize) -> &'static str {
        if error_count > 0 {
            return MODEL_FLASH_LITE; 
        }
        match turn {
            0..=4 => MODEL_FLASH_LITE, 
            _     => MODEL_31B,        
        }
    }

    pub fn failover_model(current_model: &str) -> &'static str {
        match current_model {
            MODEL_31B => MODEL_FLASH_LITE,         // Tier 1 fails -> Tier 2
            MODEL_FLASH_LITE => MODEL_26B_MOE,     // Tier 2 fails -> Tier 3
            _ => MODEL_FLASH_LITE,                 // Default safe recovery
        }
    }
}

impl Default for AgentFactory {
    fn default() -> Self { Self::new() }
}
