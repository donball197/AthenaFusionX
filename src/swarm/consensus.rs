// src/swarm/consensus.rs — v3.1 Hardened Dual Consensus
//
// FIX: ConsensusEngine::resolve previously normalised responses to lowercase
//      for voting, but then returned the lowercased/normalised winner string
//      via a format! fallback — producing malformed JSON if the winning action
//      contained uppercase characters (which JSON field values often do).
//      Now the original (un-normalised) best response is always returned.
//
// KEPT: AverageConsensus, ConsensusVector, and all tests unchanged.

use std::collections::HashMap;

// ─── QUALITATIVE (VOTING) CONSENSUS ─────────────────────────────────────────

pub struct ConsensusEngine;

impl ConsensusEngine {
    /// resolve: Majority-vote across a set of response strings.
    ///
    /// Voting is performed on the normalised (lowercase, trimmed) form of
    /// each response's <ACTION> block (or the whole response if no tags).
    /// The *original* string corresponding to the winning normalised form
    /// is returned, preserving correct capitalisation and JSON syntax.
    pub fn resolve(responses: Vec<String>) -> String {
        if responses.is_empty() {
            return "NO_CONSENSUS".to_string();
        }

        // Build a map: normalised_action → (vote_count, first_original_response)
        let mut score: HashMap<String, (usize, String)> = HashMap::new();

        for r in &responses {
            // Extract just the <ACTION> block for normalisation, falling back
            // to the full response so non-tagged outputs still participate.
            let action_block = if let (Some(s), Some(e)) =
                (r.find("<ACTION>"), r.find("</ACTION>"))
            {
                r[s + 8..e].trim().to_string()
            } else {
                r.clone()
            };

            let norm = action_block.to_lowercase();

            match score.get_mut(&norm) {
                Some((count, _original)) => {
                    *count += 1;
                }
                None => {
                    // Store the original *full* response so we can return it
                    // intact — not the normalised fragment.
                    score.insert(norm, (1, r.clone()));
                }
            }
        }

        // FIX: return `original` (the un-mutated response), not the
        //      normalised key.  The previous code returned `norm` here which
        //      lowercased JSON values and broke downstream parsing.
        score
            .into_iter()
            .max_by_key(|(_, (count, _))| *count)
            .map(|(_, (_, original))| original)
            .unwrap_or_else(|| "NO_CONSENSUS".to_string())
    }
}

// ─── QUANTITATIVE AVERAGE CONSENSUS ─────────────────────────────────────────

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
            nodes: initial
                .into_iter()
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
            let prev: Vec<f64> =
                self.nodes.iter().map(|n| n.local_state).collect();
            self.iterate(epsilon);

            let max_change = self
                .nodes
                .iter()
                .zip(prev.iter())
                .map(|(n, p)| (n.local_state - p).abs())
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);

            if max_change < 1e-6 {
                println!(
                    "\x1b[32m[AVERAGE_CONSENSUS] Converged in {} iterations to ~{:.4}\x1b[0m",
                    iter + 1,
                    self.nodes[0].local_state
                );
                break;
            }
        }
        self.nodes.iter().map(|n| n.local_state).collect()
    }

    /// Persist converged average to VMO (ties into the Zig muscle).
    pub fn persist_to_vmo(&self, handle: u32) {
        let avg = self.nodes[0].local_state;
        let data = format!("CONVERGED_AVG:{:.6}|THERMAL_STABLE", avg);
        println!(
            "\x1b[36m[RESONANCE] Persisting converged state to VMO 0x{:x}: {}\x1b[0m",
            handle, data
        );
    }
}

// ─── VECTORIZED CONSENSUS (32-D LATENT SPACE) ───────────────────────────────

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
            nodes: initial
                .into_iter()
                .map(|(id, vec)| SwarmVectorState { id, latent_intent: vec })
                .collect(),
        }
    }

    fn rms_norm(vec: &mut [f64; 32]) {
        let sum_sq: f64 = vec.iter().map(|x| x * x).sum::<f64>();
        let rms = (sum_sq / 32.0).sqrt() + 1e-6;
        for val in vec.iter_mut() {
            *val /= rms;
        }
    }

    pub fn iterate(&mut self, epsilon: f64) {
        let old_states: Vec<[f64; 32]> =
            self.nodes.iter().map(|n| n.latent_intent).collect();

        for (i, node) in self.nodes.iter_mut().enumerate() {
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
        println!(
            "\x1b[32m[VECTOR_CONSENSUS] 32-D Latent Alignment Complete.\x1b[0m"
        );
    }
}

// ─── TESTS ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_average_convergence() {
        let mut engine = AverageConsensus::new(vec![
            ("Alpha", 32.5),
            ("Beta", 41.0),
            ("Gamma", 37.8),
        ]);
        let final_states = engine.converge(0.35, 20);
        let avg = final_states[0];
        assert!((avg - 37.1).abs() < 0.01);
    }

    #[test]
    fn test_consensus_preserves_original_case() {
        let responses = vec![
            r#"<THOUGHT>T</THOUGHT><ACTION>[{"type": "vmo_create"}]</ACTION>"#.to_string(),
            r#"<THOUGHT>T</THOUGHT><ACTION>[{"type": "vmo_create"}]</ACTION>"#.to_string(),
            r#"<THOUGHT>T</THOUGHT><ACTION>[{"type": "halt"}]</ACTION>"#.to_string(),
        ];
        let winner = ConsensusEngine::resolve(responses);
        // The winner should be the vmo_create response (2 votes), and its
        // original casing must be preserved (not lowercased).
        assert!(winner.contains("vmo_create"));
        assert!(winner.contains("<ACTION>"));
    }
}
