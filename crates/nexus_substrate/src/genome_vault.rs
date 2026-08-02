use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenomeRecord {
    pub objective: String,
    pub wasm_b64: String,
    pub composite_score: f32,
    pub execution_time_ms: u64,
    pub memory_delta_kb: f32,
    pub thermal_impact: f32,
    pub timestamp: i64,
}

pub struct GenomeVault {
    genomes: Arc<Mutex<HashMap<String, GenomeRecord>>>,
    max_retain: usize,
}

impl GenomeVault {
    pub fn new(max_retain: usize) -> Self {
        Self {
            genomes: Arc::new(Mutex::new(HashMap::new())),
            max_retain,
        }
    }

    pub async fn store_high_fitness(&self, objective: &str, record: GenomeRecord) {
        let mut guard = self.genomes.lock().await;
        guard.insert(objective.to_string(), record);

        if guard.len() > self.max_retain {
            let mut sorted: Vec<_> = guard.drain().collect();
            sorted.sort_by(|a, b| b.1.composite_score.partial_cmp(&a.1.composite_score).unwrap());
            sorted.truncate(self.max_retain);
            *guard = sorted.into_iter().collect();
        }
    }

    pub async fn get_best_for_objective(&self, objective: &str) -> Option<GenomeRecord> {
        let guard = self.genomes.lock().await;
        guard.get(objective).cloned()
    }

    pub async fn get_global_best(&self) -> Option<GenomeRecord> {
        let guard = self.genomes.lock().await;
        guard.values()
            .max_by(|a, b| a.composite_score.partial_cmp(&b.composite_score).unwrap())
            .cloned()
    }
}

impl Default for GenomeVault {
    fn default() -> Self {
        Self::new(50)
    }
}
