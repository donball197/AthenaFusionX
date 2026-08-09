//! Executive Cortex – Phase 8B v2
//! Staged decision pipeline:
//!   observe → evaluate_policy → evaluate_goals → evaluate_risk → choose → emit

use super::event_bus::{CognitiveEvent, CognitiveEventKind, EventBus, EventMeta};
use super::policy_engine::PolicyEngine;
use super::traits::*;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone)]
pub struct ExecutiveCortex {
    bus: EventBus,
    policy: PolicyEngine,
}

#[derive(Debug, Default)]
struct Evaluation {
    policy_ok: bool,
    goal_pressure: f32,   // 0 = none, 1 = urgent
    risk: f32,            // 0 = safe, 1 = critical
    resource_pressure: f32,
}

impl ExecutiveCortex {
    pub fn new(bus: EventBus) -> Self {
        Self {
            bus,
            policy: PolicyEngine::new(),
        }
    }

    pub fn with_policy(bus: EventBus, policy: PolicyEngine) -> Self {
        Self { bus, policy }
    }

    // ── Pipeline stages ────────────────────────────────────────────────────

    fn observe(&self, state: &MissionState) -> Evaluation {
        let mut ev = Evaluation::default();
        ev.policy_ok = true; // policy checked against actions, not mission state
        ev.risk = 1.0 - state.health.clamp(0.0, 1.0);
        ev.resource_pressure = 0.0; // filled by scheduler later
        ev.goal_pressure = state.priority.clamp(0.0, 1.0);
        ev
    }

    fn evaluate_policy(&self, ev: &mut Evaluation, _state: &MissionState) {
        // Placeholder: mission-level policy always passes in 8B.
        // Action-level checks happen via PolicyEngine::check_action.
        ev.policy_ok = true;
    }

    fn evaluate_goals(&self, ev: &mut Evaluation, state: &MissionState) {
        // Higher priority + active status → higher pressure to continue.
        if state.status == MissionStatus::Active {
            ev.goal_pressure = state.priority.clamp(0.0, 1.0);
        } else {
            ev.goal_pressure = 0.0;
        }
    }

    fn evaluate_risk(&self, ev: &mut Evaluation, state: &MissionState) {
        ev.risk = 1.0 - state.health.clamp(0.0, 1.0);
    }

    fn choose(&self, ev: &Evaluation, state: &MissionState) -> (ExecutiveDecisionKind, String, f32) {
        match state.status {
            MissionStatus::Completed | MissionStatus::Failed | MissionStatus::Aborted => {
                (ExecutiveDecisionKind::Reflect, "mission terminal".into(), 0.95)
            }
            MissionStatus::Paused => {
                (ExecutiveDecisionKind::Continue, "resume paused mission".into(), 0.7)
            }
            MissionStatus::Active => {
                if !ev.policy_ok {
                    (ExecutiveDecisionKind::Abort, "policy violation".into(), 0.9)
                } else if ev.risk > 0.75 {
                    (
                        ExecutiveDecisionKind::Abort,
                        format!("risk critical ({:.2})", ev.risk),
                        0.9,
                    )
                } else if ev.risk > 0.5 {
                    (
                        ExecutiveDecisionKind::Replan,
                        format!("risk elevated ({:.2})", ev.risk),
                        0.8,
                    )
                } else if ev.resource_pressure > 0.8 {
                    (
                        ExecutiveDecisionKind::Pause,
                        "resource pressure high".into(),
                        0.75,
                    )
                } else {
                    (ExecutiveDecisionKind::Continue, "mission healthy".into(), 0.85)
                }
            }
        }
    }

    // ── Public API ─────────────────────────────────────────────────────────

    pub async fn decide(&self, state: &MissionState) -> ExecutiveDecision {
        let mut ev = self.observe(state);
        self.evaluate_policy(&mut ev, state);
        self.evaluate_goals(&mut ev, state);
        self.evaluate_risk(&mut ev, state);
        let (kind, rationale, confidence) = self.choose(&ev, state);

        let decision = ExecutiveDecision {
            kind,
            reason: DecisionReason {
                confidence,
                supporting_evidence: vec![
                    format!("health={:.2}", state.health),
                    format!("risk={:.2}", ev.risk),
                    format!("goal_pressure={:.2}", ev.goal_pressure),
                ],
                policy_trigger: Some("phase8b_v2_pipeline".into()),
                expected_cost: 0.0,
                rationale,
            },
        };

        self.bus
            .publish(CognitiveEvent {
                meta: EventMeta::new("executive_cortex"),
                mission_id: Some(state.id),
                kind: CognitiveEventKind::DecisionMade {
                    decision_summary: format!("{:?}", decision.kind),
                },
            })
            .await;

        decision
    }

    pub fn start_mission(&self, directive: &str, priority: f32) -> MissionState {
        MissionState {
            id: Uuid::new_v4(),
            directive: directive.to_string(),
            priority,
            health: 1.0,
            status: MissionStatus::Active,
            intent: Some(directive.to_string()),
        }
    }

    pub fn policy(&self) -> &PolicyEngine {
        &self.policy
    }
}
