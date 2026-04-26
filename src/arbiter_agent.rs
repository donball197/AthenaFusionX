// src/arbiter_agent.rs — v3.5 [NORMALIZED_PATTERNS]
use crate::neural_agent::MicroAgent;
use std::sync::Arc;

pub struct ArbiterAgent {
    pub agent: Arc<MicroAgent>,
}

impl ArbiterAgent {
    pub fn new(agent: Arc<MicroAgent>) -> Self {
        Self { agent }
    }

    /// validate_action: All patterns are pre-lowercased to match normalization.
    pub fn validate_action(action_type: &str, command: &str) -> Result<(), String> {
        // All patterns must be lowercase — command is normalized before matching.
        let dangerous_patterns = [
            "rm -rf /",
            "mkfs",
            "> /dev/sda",
            "chmod 777",
            "/etc/shadow",
            "mv /etc/",
            ":(){ :|:& };:",
            "dd if=/dev/zero",
            "fork bomb",
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
            return Err(
                "SANDBOX_VIOLATION: Directory traversal attempt blocked.".to_string(),
            );
        }

        Ok(())
    }

    /// evaluate_entropy_delta: Blocks writes whose entropy shift exceeds threshold.
    pub fn evaluate_entropy_delta(base_entropy: f32, new_entropy: f32) -> Result<(), String> {
        let delta = (new_entropy - base_entropy).abs();
        if delta > 0.5 {
            return Err(format!(
                "ENTROPY_ANOMALY: Delta of {:.4} exceeds 0.5 threshold. VMO write aborted.",
                delta
            ));
        }
        Ok(())
    }

    pub fn resolve_consensus(alpha_vector: &[f32], beta_vector: &[f32]) -> bool {
        if alpha_vector.len() != beta_vector.len() {
            return false;
        }
        let diff: f32 = alpha_vector
            .iter()
            .zip(beta_vector.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        diff < 0.5
    }
}
