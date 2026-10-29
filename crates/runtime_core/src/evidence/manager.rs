use crate::epistemic_engine::EpistemicEngine;
use crate::evidence::{BeliefVector, Evidence, EvidenceKind, Observation};
use reqwest::Client;
use uuid::Uuid;

/// Central translation layer between the RoboticExecutor and the EvidenceGraph.
///
/// - Fast path  : pure heuristic (microseconds)
/// - Slow path  : optional LLM escalation for truly ambiguous observations
#[derive(Debug, Default)]
pub struct EvidenceManager;

impl EvidenceManager {
    pub fn new() -> Self {
        Self
    }

    /// Synchronous, zero-network path (kept for compatibility).
    pub fn process_observation(&self, obs: Observation, strategy_id: Option<Uuid>) -> Evidence {
        let (kind, failure) = Evidence::classify(&obs.raw, obs.success);
        let belief_delta = self.compute_belief_delta(&obs, kind);
        let mut ev = Evidence::from_observation(obs, kind, belief_delta, strategy_id);
        ev.observation.failure_kind = failure;
        ev
    }

    /// Async path with smart escalation.
    pub async fn process_observation_with_escalation(
        &self,
        obs: Observation,
        strategy_id: Option<Uuid>,
        client: &Client,
        api_key: &str,
    ) -> Evidence {
        let (kind, failure) = Evidence::classify(&obs.raw, obs.success);

        // Fast path
        if !Self::should_escalate(&obs, kind) {
            let belief_delta = self.compute_belief_delta(&obs, kind);
            let mut ev = Evidence::from_observation(obs, kind, belief_delta, strategy_id);
            ev.observation.failure_kind = failure;
            return ev;
        }

        // Slow path
        println!(
            "\x1b[35m>> [EVIDENCE] Ambiguous observation ({} chars) — escalating to light critic\x1b[0m",
            obs.raw.len()
        );

        match EpistemicEngine::evaluate_evidence(
            client,
            api_key,
            "Analyze observation",
            &obs.source,
            &obs.raw,
        )
        .await
        {
            Ok(response) => {
                // Map the 1D float back into a multi-dimensional BeliefVector
                let val = response.confidence_modifier;
                let delta = BeliefVector::new(val * 0.5, val, val * -0.2, val * 0.3);
                let mut ev = Evidence::from_observation(obs, kind, delta, strategy_id);
                ev.observation.failure_kind = failure;
                ev
            }
            Err(e) => {
                println!(
                    "\x1b[33m>> [EVIDENCE] Escalation failed ({}) — falling back to Neutral/ZERO\x1b[0m",
                    e
                );
                let mut ev = Evidence::from_observation(
                    obs,
                    EvidenceKind::Neutral,
                    BeliefVector::ZERO,
                    strategy_id,
                );
                ev.observation.failure_kind = failure;
                ev
            }
        }
    }

    fn should_escalate(obs: &Observation, kind: EvidenceKind) -> bool {
        match kind {
            EvidenceKind::CommandSuccess
            | EvidenceKind::CommandFailure
            | EvidenceKind::SecurityViolation
            | EvidenceKind::FileRead
            | EvidenceKind::FileWrite
            | EvidenceKind::ArchitecturalSmell
            | EvidenceKind::PatternMatch
            | EvidenceKind::PositiveSignal => false,
            EvidenceKind::Neutral => obs.raw.len() > 500,
        }
    }

    fn compute_belief_delta(&self, obs: &Observation, kind: EvidenceKind) -> BeliefVector {
        match kind {
            EvidenceKind::SecurityViolation => BeliefVector::new(-0.15, -0.10, 0.55, -0.85),
            EvidenceKind::CommandSuccess => {
                if obs.source.contains("read") || obs.raw.contains("[READ_OK]") {
                    BeliefVector::new(0.18, 0.42, -0.08, 0.12)
                } else {
                    BeliefVector::new(0.22, 0.28, -0.05, 0.10)
                }
            }
            EvidenceKind::CommandFailure => BeliefVector::new(-0.25, 0.15, 0.20, -0.05),
            EvidenceKind::FileRead => BeliefVector::new(0.12, 0.45, -0.05, 0.08),
            EvidenceKind::FileWrite => BeliefVector::new(0.20, 0.30, 0.10, -0.05),
            EvidenceKind::PatternMatch => BeliefVector::new(0.15, 0.35, -0.05, 0.05),
            EvidenceKind::ArchitecturalSmell => BeliefVector::new(-0.18, 0.20, 0.40, -0.08),
            EvidenceKind::PositiveSignal => BeliefVector::new(0.18, 0.25, -0.08, 0.10),
            EvidenceKind::Neutral => BeliefVector::ZERO,
        }
    }
}
