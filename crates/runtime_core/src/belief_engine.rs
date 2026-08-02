use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefState {
    pub id: Uuid,

    pub proposition: String,

    pub confidence: f32,

    pub evidence_count: usize,

    pub stability: f32,
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

    pub fn update(&self, current: Option<BeliefState>, evidence: BeliefEvidence) -> BeliefState {
        match current {
            Some(mut belief) => {
                let adjustment = evidence.reliability * 0.1;

                belief.confidence = (belief.confidence + adjustment).min(1.0);

                belief.evidence_count += 1;

                belief.stability = (belief.stability + 0.05).min(1.0);

                belief
            }

            None => BeliefState {
                id: Uuid::new_v4(),

                proposition: evidence.observation,

                confidence: evidence.reliability,

                evidence_count: 1,

                stability: 0.5,
            },
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
        self.confidence += delta;

        self.confidence = self.confidence.clamp(0.0, 1.0);
    }
}

impl BeliefState {
    pub fn apply_delta(&mut self, delta: crate::belief_delta::BeliefDelta) {
        self.confidence = delta.new_confidence;
    }
}
