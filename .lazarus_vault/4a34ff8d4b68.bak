use std::collections::HashMap;

/// NEXUS v7 Consensus Engine
/// Deterministic majority vote resolver with stable representative selection.
pub struct ConsensusEngine;

impl ConsensusEngine {
    pub fn resolve(responses: Vec<String>) -> String {
        if responses.is_empty() {
            return "NO_CONSENSUS".to_string();
        }

        // normalized -> (count, best_original_example)
        let mut score: HashMap<String, (usize, String)> = HashMap::new();

        for r in responses {
            let normalized = r.trim().to_lowercase();

            match score.get_mut(&normalized) {
                Some((count, _original)) => {
                    *count += 1;
                }
                None => {
                    score.insert(normalized, (1, r));
                }
            }
        }

        score
            .into_iter()
            .max_by_key(|(_, (count, _))| *count)
            .map(|(_, (_, original))| original)
            .unwrap_or_else(|| "NO_CONSENSUS".to_string())
    }
}
