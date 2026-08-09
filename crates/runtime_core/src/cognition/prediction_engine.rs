use super::traits::{Prediction, PredictionEvaluation};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct ActionHistory {
    pub success_count: u32,
    pub total_count: u32,
    pub avg_cost: f32,
}

/// V5 PredictionEngine: Operates a fast nearest-neighbor lookup on historical logs
/// using action-verb + target-extension signatures.
#[derive(Clone, Default)]
pub struct PredictionEngine {
    active_predictions: Arc<RwLock<HashMap<Uuid, Prediction>>>,
    history: Arc<RwLock<HashMap<String, ActionHistory>>>,
}

impl PredictionEngine {
    pub fn new() -> Self {
        Self {
            active_predictions: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generates a heuristic signature: e.g. "read_file src/main.rs" -> "read_file_rs"
    fn get_signature(action_str: &str) -> String {
        let parts: Vec<&str> = action_str.split_whitespace().collect();
        let verb = parts.first().unwrap_or(&"unknown");
        let ext = parts.get(1).and_then(|p| p.split('.').next_back()).unwrap_or("none");
        format!("{}_{}", verb, ext)
    }

    pub fn generate_prediction(&self, action_id: Uuid, action_str: &str) -> Prediction {
        let sig = Self::get_signature(action_str);
        let hist_lock = self.history.read();
        
        let (confidence, cost) = if let Some(h) = hist_lock.get(&sig) {
            let conf = if h.total_count > 0 { h.success_count as f32 / h.total_count as f32 } else { 0.5 };
            (conf, h.avg_cost)
        } else {
            (0.5, 0.1) // Baseline default
        };

        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let pred = Prediction {
            action_id,
            expected_state: sig, // We store the signature here to match against upon evaluation
            confidence,
            cost_estimate: cost,
            timestamp: ts,
        };
        
        self.active_predictions.write().insert(action_id, pred.clone());
        pred
    }

    pub fn evaluate_prediction(&self, action_id: Uuid, actual_cost: f32, success: bool) -> PredictionEvaluation {
        let mut active = self.active_predictions.write();
        if let Some(pred) = active.remove(&action_id) {
            let mut hist_lock = self.history.write();
            let h = hist_lock.entry(pred.expected_state.clone()).or_default();
            
            h.total_count += 1;
            if success { h.success_count += 1; }
            
            // Rolling average update
            h.avg_cost = (h.avg_cost * (h.total_count - 1) as f32 + actual_cost) / h.total_count as f32;
            
            let accuracy = if success == (pred.confidence >= 0.5) { 1.0 } else { 0.0 };
            let error = (actual_cost - pred.cost_estimate).abs();
            
            PredictionEvaluation { accuracy, error }
        } else {
            PredictionEvaluation { accuracy: 0.0, error: 0.0 }
        }
    }
}
