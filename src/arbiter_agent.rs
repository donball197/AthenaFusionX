// src/arbiter_agent.rs — v3.7 [BFT_STABILIZED]
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
            "/etc/shadow", "mv /etc/", ":(){ :|:& };:", "dd if=/dev/zero", "fork bomb",
        ];
        let normalized = command.to_lowercase();
        for pattern in dangerous_patterns.iter() {
            if normalized.contains(pattern) {
                return Err(format!("SANDBOX_VIOLATION: Critical threat pattern '{}' detected in {}.", pattern, action_type));
            }
        }
        if normalized.contains("../../../") {
            return Err("SANDBOX_VIOLATION: Directory traversal attempt blocked.".to_string());
        }
        Ok(())
    }

    pub fn evaluate_entropy_delta(base_entropy: f32, new_entropy: f32) -> Result<(), String> {
        let delta = (new_entropy - base_entropy).abs();
        if delta > 0.5 {
            return Err(format!("ENTROPY_ANOMALY: Delta of {:.4} exceeds 0.5 threshold.", delta));
        }
        Ok(())
    }

    pub fn resolve_consensus(alpha_vector: &[f32], beta_vector: &[f32]) -> bool {
        if alpha_vector.len() != beta_vector.len() { return false; }
        let diff: f32 = alpha_vector.iter().zip(beta_vector.iter()).map(|(a, b)| (a - b).abs()).sum();
        diff < 0.5
    }

    /// bft_consensus: Byzantine fault-tolerant consensus over N embedding vectors.
    /// Requires strictly more than 2/3 of nodes to agree within L1-distance epsilon.
    pub fn bft_consensus(vectors: &[Vec<f32>], epsilon: f32) -> Option<Vec<f32>> {
        if vectors.is_empty() { return None; }
        let n = vectors.len();
        let threshold = (2 * n / 3) + 1;
        let dim = vectors[0].len();
        
        if vectors.iter().any(|v| v.len() != dim) {
            return None;
        }

        let (best_idx, neighbor_count) = (0..n).map(|i| {
            let neighbors = (0..n).filter(|&j| {
                vectors[i].iter().zip(vectors[j].iter()).map(|(a, b)| (a - b).abs()).sum::<f32>() < epsilon
            }).count();
            (i, neighbors)
        }).max_by_key(|&(_, count)| count)?;

        if neighbor_count < threshold {
            return None;
        }

        let agreeing: Vec<&Vec<f32>> = (0..n).filter(|&j| {
            vectors[best_idx].iter().zip(vectors[j].iter()).map(|(a, b)| (a - b).abs()).sum::<f32>() < epsilon
        }).map(|j| &vectors[j]).collect();

        let centroid: Vec<f32> = (0..dim).map(|k| {
            agreeing.iter().map(|v| v[k]).sum::<f32>() / agreeing.len() as f32
        }).collect();

        Some(centroid)
    }
}

#[cfg(test)]
mod bft_tests {
    use super::*;
    #[test]
    fn unanimous_returns_centroid() {
        let vecs = vec![ vec![1.0, 2.0], vec![1.1, 2.1], vec![0.9, 1.9] ];
        assert!(ArbiterAgent::bft_consensus(&vecs, 0.5).is_some());
    }
}
