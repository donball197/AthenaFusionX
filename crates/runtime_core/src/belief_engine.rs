use crate::evidence::BeliefVector;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefState {
    pub id: Uuid,
    pub proposition: String,
    pub confidence: f32,
    pub evidence_count: usize,
    pub stability: f32,
    /// Rich multidimensional view (canonical path going forward).
    pub vector: BeliefVector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefEvidence {
    pub source: String,
    pub observation: String,
    pub reliability: f32,
}

pub struct BeliefEngine;

impl BeliefEngine {
    pub fn new() -> Self {
        Self
    }

    /// Compatibility entry point – routes through BeliefVector::bayesian_update.
    pub fn update(&self, current: Option<BeliefState>, evidence: BeliefEvidence) -> BeliefState {
        let likelihood = evidence.reliability.clamp(0.0, 1.0);
        match current {
            Some(mut belief) => {
                // Canonical update
                belief.vector = belief.vector.bayesian_update(likelihood, 0.85);
                // Keep scalar fields in sync for legacy callers
                belief.confidence = belief.vector.scalar_score();
                belief.evidence_count += 1;
                belief.stability = (belief.stability + 0.05).min(1.0);
                belief
            }
            None => {
                let vector = BeliefVector::default().bayesian_update(likelihood, 0.85);
                BeliefState {
                    id: Uuid::new_v4(),
                    proposition: evidence.observation,
                    confidence: vector.scalar_score(),
                    evidence_count: 1,
                    stability: 0.5,
                    vector,
                }
            }
        }
    }
}

impl Default for BeliefEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl BeliefState {
    pub fn update_confidence(&mut self, delta: f32) {
        self.confidence = (self.confidence + delta).clamp(0.0, 1.0);
    }

    pub fn apply_delta(&mut self, delta: crate::belief_delta::BeliefDelta) {
        self.confidence = delta.new_confidence;
        // Keep vector roughly in sync
        self.vector.implementation_confidence = delta.new_confidence;
    }

    /// Hard gate – prefer this over raw confidence checks.
    pub fn may_prove(&self, required_artifacts: u8, threshold: f32) -> bool {
        self.vector.may_prove(required_artifacts, threshold)
    }
}
