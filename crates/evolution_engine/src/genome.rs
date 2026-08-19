use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveWeights {
    pub meta_reasoning_threshold: f32, // When to trigger GoT
    pub critic_penalty_weight: f32,    // How heavily to penalize vetoes
    pub heuristic_reward: f32,         // Reward for physical exploration
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemGenome {
    pub generation: u32,
    pub hash: String,
    pub weights: CognitiveWeights,
    pub active_wasm_modules: Vec<String>,
    pub primary_heavy_model: String,
    pub primary_light_model: String,
    pub fitness_score: f32,
}

impl SystemGenome {
    pub fn baseline() -> Self {
        let heavy = std::env::var("NEXUS_HEAVY_MODEL")
            .unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b".to_string());
        let light = std::env::var("NEXUS_LIGHT_MODEL")
            .unwrap_or_else(|_| "gemini-3.5-flash-lite".to_string());

        Self {
            generation: 0,
            hash: "genesis_0000".to_string(),
            weights: CognitiveWeights {
                meta_reasoning_threshold: 0.60,
                critic_penalty_weight: -0.30,
                heuristic_reward: 0.30,
            },
            active_wasm_modules: vec![],
            primary_heavy_model: heavy,
            primary_light_model: light,
            fitness_score: 100.0,
        }
    }
}
