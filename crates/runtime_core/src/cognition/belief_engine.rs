use std::collections::HashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct WeightedFact {
    pub confidence: f32,
    pub last_updated: u64,
}

/// V5 BeliefEngine: Maintains persistent environment facts (e.g., "gemini_api_healthy")
/// with automatic temporal decay for staleness.
#[derive(Clone, Default)]
pub struct BeliefEngine {
    facts: Arc<RwLock<HashMap<String, WeightedFact>>>,
}

impl BeliefEngine {
    pub fn new() -> Self {
        Self { facts: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Modifies a belief based on real-world evidence. 
    /// Applies decay calculation before adjustment.
    pub fn update_belief(&self, key: &str, delta: f32) -> f32 {
        let mut lock = self.facts.write();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let fact = lock.entry(key.to_string()).or_insert(WeightedFact { confidence: 0.5, last_updated: now });
        
        let elapsed_hours = now.saturating_sub(fact.last_updated) as f32 / 3600.0;
        let decay = elapsed_hours * 0.05; // 5% decay per hour of staleness
        
        fact.confidence = (fact.confidence - decay + delta).clamp(0.0, 1.0);
        fact.last_updated = now;
        fact.confidence
    }

    /// Retrieves current confidence of a belief, factoring in decay since last update.
    pub fn get_belief(&self, key: &str) -> f32 {
        let lock = self.facts.read();
        if let Some(fact) = lock.get(key) {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let elapsed_hours = now.saturating_sub(fact.last_updated) as f32 / 3600.0;
            let decay = elapsed_hours * 0.05;
            (fact.confidence - decay).clamp(0.0, 1.0)
        } else {
            0.5 // Unknown facts rest at neutral parity
        }
    }
}
