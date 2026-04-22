//! NEXUS SWARM — consensus.rs v2.1
//! FIX: unwrap() → unwrap_or(Ordering::Equal) in all PartialOrd sites.
//!      Restored ConsensusEngine::resolve() used by swarm_core.
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct ConsensusEngine {
    pub epoch: u64,
    pub weight: f64,
}

impl ConsensusEngine {
    /// compare_weights: Ordering with graceful fallback for NaN / non-comparable.
    pub fn compare_weights(&self, other: &Self) -> Ordering {
        self.weight
            .partial_cmp(&other.weight)
            .unwrap_or(Ordering::Equal)
    }

    /// resolve: Pick the "best" string from a Vec by length heuristic.
    /// Used by swarm_core::SwarmCore::run.
    pub fn resolve(outputs: Vec<String>) -> String {
        outputs
            .into_iter()
            .max_by(|a, b| a.len().cmp(&b.len()))
            .unwrap_or_else(|| "NO_CONSENSUS".to_string())
    }
}

/// AverageConsensus: Distributed thermal averaging for swarm_sim binary.
#[derive(Debug, Clone)]
pub struct ConsensusNode {
    pub name: String,
    pub local_state: f64,
}

pub struct AverageConsensus {
    pub nodes: Vec<ConsensusNode>,
}

impl AverageConsensus {
    pub fn new(initial: Vec<(&str, f64)>) -> Self {
        Self {
            nodes: initial
                .into_iter()
                .map(|(name, state)| ConsensusNode {
                    name: name.to_string(),
                    local_state: state,
                })
                .collect(),
        }
    }

    /// iterate: One step of the average-consensus algorithm (epsilon = mixing rate).
    pub fn iterate(&mut self, epsilon: f64) {
        let mean: f64 =
            self.nodes.iter().map(|n| n.local_state).sum::<f64>() / self.nodes.len() as f64;
        for node in self.nodes.iter_mut() {
            node.local_state += epsilon * (mean - node.local_state);
        }
    }

    /// converge: Run up to max_iterations until all nodes are within 0.01 of each other.
    pub fn converge(&mut self, epsilon: f64, max_iterations: usize) {
        for _ in 0..max_iterations {
            let min = self.nodes.iter().map(|n| n.local_state).fold(f64::INFINITY, f64::min);
            let max = self
                .nodes
                .iter()
                .map(|n| n.local_state)
                .fold(f64::NEG_INFINITY, f64::max);
            if (max - min).abs() < 0.01 {
                break;
            }
            self.iterate(epsilon);
        }
    }

    /// persist_to_vmo: Stub — writes consensus result to kernel VMO handle.
    pub fn persist_to_vmo(&self, handle: usize) {
        let result = self.nodes.first().map(|n| n.local_state).unwrap_or(0.0);
        println!(
            "\x1b[36m[CONSENSUS] Persisting result {:.4} to VMO handle 0x{:x}\x1b[0m",
            result, handle
        );
    }
}
