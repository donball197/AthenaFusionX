use crate::evidence::{BeliefVector, Evidence};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// EvidenceGraph – a lightweight, strategy-keyed store of Evidence nodes.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EvidenceGraph {
    /// strategy_id → list of Evidence that support / refute it
    by_strategy: HashMap<Uuid, Vec<Evidence>>,

    /// Global chronological log (useful for later dream-cycle / audit)
    chronology: Vec<Uuid>,
}

impl EvidenceGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a fully materialised Evidence node.
    pub fn insert(&mut self, evidence: Evidence) {
        if let Some(strategy_id) = evidence.related_strategy_id {
            self.by_strategy
                .entry(strategy_id)
                .or_default()
                .push(evidence.clone());
        }
        self.chronology.push(evidence.id);
    }

    /// Aggregate all evidence belonging to a strategy into a single BeliefVector.
    pub fn aggregate_belief(
        &self,
        strategy_id: Uuid,
        baseline: BeliefVector,
    ) -> BeliefVector {
        let Some(evidences) = self.by_strategy.get(&strategy_id) else {
            return baseline;
        };

        let mut current = baseline;

        for ev in evidences {
            let weight = (0.35 + ev.belief_delta.evidence_quality * 0.55).clamp(0.15, 0.85);
            current = current.update(&ev.belief_delta, weight);
        }

        current
    }

    pub fn evidence_count(&self, strategy_id: Uuid) -> usize {
        self.by_strategy
            .get(&strategy_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    pub fn evidence_for(&self, strategy_id: Uuid) -> &[Evidence] {
        self.by_strategy
            .get(&strategy_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn chronology(&self) -> &[Uuid] {
        &self.chronology
    }
}
