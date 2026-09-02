//! Epistemic Tuple Blackboard: Immutable, typed fact exchange for AthenaFusionX.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpistemicTuple {
    pub namespace: String,   // "AST" | "COMPILER" | "PROOF" | "INTENT"
    pub subject: String,     // e.g. "router.rs" or "cargo_check"
    pub predicate: String,   // e.g. "SYNTAX_VALID" | "COMPILE_OK" | "VULNERABILITY_ELIMINATED"
    pub payload: String,     // Exact content or AST slice
    pub confidence: f32,     // 0.0 ..= 1.0
    pub timestamp: u64,
}

impl EpistemicTuple {
    pub fn new(namespace: &str, subject: &str, predicate: &str, payload: &str, confidence: f32) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            namespace: namespace.to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            payload: payload.to_string(),
            confidence: confidence.clamp(0.0, 1.0),
            timestamp,
        }
    }
}

#[derive(Clone, Default)]
pub struct EpistemicBlackboard {
    tuples: Arc<RwLock<HashMap<String, Vec<EpistemicTuple>>>>,
}

impl EpistemicBlackboard {
    pub fn new() -> Self {
        Self {
            tuples: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn post(&self, tuple: EpistemicTuple) {
        let mut map = self.tuples.write().await;
        let key = format!("{}:{}", tuple.namespace, tuple.subject);
        map.entry(key).or_default().push(tuple);
    }

    pub async fn query_subject(&self, namespace: &str, subject: &str) -> Vec<EpistemicTuple> {
        let map = self.tuples.read().await;
        let key = format!("{}:{}", namespace, subject);
        map.get(&key).cloned().unwrap_or_default()
    }

    pub async fn has_proven_fact(&self, namespace: &str, subject: &str, predicate: &str) -> bool {
        let facts = self.query_subject(namespace, subject).await;
        facts.iter().any(|f| f.predicate == predicate && f.confidence >= 0.85)
    }

    pub async fn snapshot_summary(&self) -> String {
        let map = self.tuples.read().await;
        let mut out = String::from("=== EPISTEMIC BLACKBOARD FACTS ===\n");
        for (key, list) in map.iter() {
            for t in list {
                out.push_str(&format!("[{}] {} -> {} (Conf: {:.2})\n", key, t.predicate, t.payload.lines().next().unwrap_or(""), t.confidence));
            }
        }
        out
    }
}
