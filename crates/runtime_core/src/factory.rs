// Project Nexus Singularity v3.0.0 - Agent Factory Namespace Realignment
// Restores strict struct association patterns for the core conductor layer

pub const MODEL_26B: &str = "gemma-4-26b-a4b-it";
pub const MODEL_31B: &str = "gemma-4-31b-it";

pub struct AgentFactory;

impl AgentFactory {
    pub fn new() -> Self {
        Self
    }

    /// Primary model routing logic based on turn depth and current errors
    pub fn get_model_for_turn(turn: usize, error_count: usize) -> &'static str {
        // If the node encounters stability resistance, escalate to 31B Dense immediately
        if error_count > 1 {
            return MODEL_31B;
        }

        // Gemma 4 26B MoE handles all primary Conductor execution routes (turns 0-6)
        match turn {
            0..=6 => MODEL_26B,
            _     => MODEL_31B,
        }
    }

    /// Failover resolution path when a specific engine configuration hits a wall
    pub fn failover_model(current_model: &str) -> &'static str {
        if current_model == MODEL_26B {
            MODEL_31B
        } else {
            MODEL_31B // Maintain dense baseline if already escalated
        }
    }
}
