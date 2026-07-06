// src/arbiter_agent.rs — v3.3 Stateful Arbiter
use crate::neural_agent::MicroAgent;
use std::sync::Arc;

pub struct ArbiterAgent {
    pub agent: Arc<MicroAgent>,
}

impl ArbiterAgent {
    pub fn new(agent: Arc<MicroAgent>) -> Self {
        Self { agent }
    }

    pub fn validate_action(action_type: &str, command: &str) -> Result<(), String> {
        let dangerous_patterns = [
            "rm -rf /", "mkfs", "> /dev/sda", "chmod 777",
            "/etc/shadow", "mv /etc/", ":(){ :|:& };:"
        ];

        let normalized = command.to_lowercase();
        for pattern in dangerous_patterns.iter() {
            if normalized.contains(pattern) {
                return Err(format!(
                    "SANDBOX_VIOLATION: Critical threat pattern '{}' detected in {}.",
                    pattern, action_type
                ));
            }
        }

        if normalized.contains("../../../") {
            return Err("SANDBOX_VIOLATION: Directory traversal attempt blocked.".to_string());
        }

        Ok(())
    }

    pub fn resolve_consensus(alpha_vector: &[f32], beta_vector: &[f32]) -> bool {
        if alpha_vector.len() != beta_vector.len() { return false; }
        let mut diff: f32 = 0.0;
        for i in 0..alpha_vector.len() {
            diff += (alpha_vector[i] - beta_vector[i]).abs();
        }
        diff < 0.5
    }
}
