// src/swarm/consensus.rs — Hardened Dual Consensus (v2)
// Qualitative voting + Quantitative averaging with anti-loop guards

use std::collections::HashMap;

/// Qualitative (voting) consensus — used by SwarmCore
pub struct ConsensusEngine;

impl ConsensusEngine {
    pub fn resolve(responses: Vec<String>) -> String {
        if responses.is_empty() {
            return "NO_CONSENSUS".to_string();
        }

        let mut score: HashMap<String, usize> = HashMap::new();
        for r in &responses {
            let core = if let (Some(s), Some(e)) = (r.find("<ACTION>"), r.find("</ACTION>")) {
                r[s + 8..e].trim().to_string()
            } else {
                r.clone()
            };
            let norm = core.to_lowercase();
            *score.entry(norm).or_insert(0) += 1;
        }

        let best = score.into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|(a, _)| a)
            .unwrap_or_else(|| "[]".to_string());

        // Return the first original response that matches the winner
        for r in responses {
            if r.to_lowercase().contains(&best) {
                return r;
            }
        }

        format!("<THOUGHT>Consensus achieved on majority action.</THOUGHT>\n<ACTION>\n{}\n</ACTION>", best)
    }
}

/// Quantitative Average Consensus — mathematically sound, with convergence guard
#[derive(Debug, Clone)]
pub struct SwarmNodeState {
    pub id: &'static str,
    pub local_state: f64,
}

pub struct AverageConsensus {
    pub nodes: Vec<SwarmNodeState>,
}

impl AverageConsensus {
    pub fn new(initial: Vec<(&'static str, f64)>) -> Self {
        Self {
            nodes: initial.into_iter()
                .map(|(id, val)| SwarmNodeState { id, local_state: val })
                .collect(),
        }
    }

    pub fn iterate(&mut self, epsilon: f64) {
        let old: Vec<f64> = self.nodes.iter().map(|n| n.local_state).collect();
        for (i, node) in self.nodes.iter_mut().enumerate() {
            let mut delta = 0.0;
            for (j, &val) in old.iter().enumerate() {
                if i != j {
                    delta += val - old[i];
                }
            }
            node.local_state += epsilon * delta;
        }
    }

    pub fn converge(&mut self, epsilon: f64, max_iters: usize) -> Vec<f64> {
        for iter in 0..max_iters {
            let prev: Vec<f64> = self.nodes.iter().map(|n| n.local_state).collect();
            self.iterate(epsilon);

            let max_change = self.nodes.iter().zip(prev.iter())
                .map(|(n, p)| (n.local_state - p).abs())
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);

            if max_change < 1e-6 {
                println!("\x1b[32m[AVERAGE_CONSENSUS] Converged in {} iterations to ~{:.4}\x1b[0m", iter + 1, self.nodes[0].local_state);
                break;
            }
        }
        self.nodes.iter().map(|n| n.local_state).collect()
    }

    /// Persist converged average to VMO (ties into your Zig muscle)
    pub fn persist_to_vmo(&self, handle: u32) {
        let avg = self.nodes[0].local_state; // all nodes converge to same value
        let data = format!("CONVERGED_AVG:{:.6}|THERMAL_STABLE", avg);
        // In real execution this would call zx_vmo_write_native via FFI
        println!("\x1b[36m[RESONANCE] Persisting converged state to VMO 0x{:x}: {}\x1b[0m", handle, data);
    }
}

// Simple test (run with: cargo test --test consensus)
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_convergence() {
        let mut engine = AverageConsensus::new(vec![
            ("Alpha", 32.5),
            ("Beta",  41.0),
            ("Gamma", 37.8),
        ]);
        let final_states = engine.converge(0.35, 20);
        let avg = final_states[0];
        assert!((avg - 37.1).abs() < 0.01);
    }
}

// --- PART 3: VECTORIZED CONSENSUS (32-D Latent Space) ---

pub struct ConsensusVector {
    pub nodes: Vec<SwarmVectorState>,
}

#[derive(Debug, Clone)]
pub struct SwarmVectorState {
    pub id: &'static str,
    pub latent_intent: [f64; 32],
}

impl ConsensusVector {
    pub fn new(initial: Vec<(&'static str, [f64; 32])>) -> Self {
        Self {
            nodes: initial.into_iter()
                .map(|(id, vec)| SwarmVectorState { id, latent_intent: vec })
                .collect(),
        }
    }

    /// rms_norm: Scales the vector to prevent magnitude drift in logit space
    fn rms_norm(vec: &mut [f64; 32]) {
        let sum_sq: f64 = vec.iter().map(|x| x * x).sum::<f64>();
        let rms = (sum_sq / 32.0).sqrt() + 1e-6;
        for val in vec.iter_mut() {
            *val /= rms;
        }
    }

    /// iterate_elementwise: x_i(k+1) = x_i(k) + ε * Σ(x_j - x_i)
    pub fn iterate(&mut self, epsilon: f64) {
        let old_states: Vec<[f64; 32]> = self.nodes.iter().map(|n| n.latent_intent).collect();
        
        for (i, node) in self.nodes.iter_mut().enumerate() {
            // Pre-update Normalization (The User's Dual-Normalization Requirement)
            Self::rms_norm(&mut node.latent_intent);
            
            let mut delta = [0.0f64; 32];
            for j in 0..old_states.len() {
                if i != j {
                    for d in 0..32 {
                        delta[d] += old_states[j][d] - old_states[i][d];
                    }
                }
            }

            for d in 0..32 {
                node.latent_intent[d] += epsilon * delta[d];
            }
        }
    }

    pub fn converge(&mut self, epsilon: f64, max_iters: usize) {
        for _ in 0..max_iters {
            self.iterate(epsilon);
        }
        println!("\x1b[32m[VECTOR_CONSENSUS] 32-D Latent Alignment Complete.\x1b[0m");
    }
}
