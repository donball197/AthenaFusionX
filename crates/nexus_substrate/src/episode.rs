//! Episode provenance for memory records.
//!
//! Up to four identifiers locate a memory in the execution hierarchy:
//!
//!   plan_id     one DAG execution (task_manager.rs)
//!   task_id     one node within the DAG
//!   session_id  one EpistemicSession (cortical_loop.rs)
//!   mission_id  one SovereignCore::execute_mission call
//!
//! All four may be None. A record written before provenance was
//! captured has all four None ("episode unknown") and is never
//! eligible for automatic current-episode recall.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeContext {
    pub plan_id: Option<String>,
    pub task_id: Option<String>,
    pub session_id: Option<String>,
    pub mission_id: Option<String>,
}

impl EpisodeContext {
    pub fn is_known(&self) -> bool {
        self.plan_id.is_some()
            || self.task_id.is_some()
            || self.session_id.is_some()
            || self.mission_id.is_some()
    }

    /// Strict session match. Both sides must have session_id.
    pub fn same_session(&self, other: &Self) -> bool {
        matches!(
            (&self.session_id, &other.session_id),
            (Some(a), Some(b)) if a == b
        )
    }

    /// Mission-level match, strongest identifier wins:
    /// plan_id -> mission_id -> task_id -> session_id.
    pub fn same_mission(&self, other: &Self) -> bool {
        if let (Some(a), Some(b)) = (&self.plan_id, &other.plan_id) {
            return a == b;
        }
        if let (Some(a), Some(b)) = (&self.mission_id, &other.mission_id) {
            return a == b;
        }
        if let (Some(a), Some(b)) = (&self.task_id, &other.task_id) {
            return a == b;
        }
        self.same_session(other)
    }

    pub fn describe_short(&self) -> String {
        let s = |x: &str| x.chars().take(8).collect::<String>();
        if let Some(p) = &self.plan_id {
            return format!("plan:{}", s(p));
        }
        if let Some(m) = &self.mission_id {
            return format!("mission:{}", s(m));
        }
        if let Some(t) = &self.task_id {
            return format!("task:{}", s(t));
        }
        if let Some(sid) = &self.session_id {
            return format!("session:{}", s(sid));
        }
        "unknown".to_string()
    }
}

/// Eligibility scope for a recall call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallScope {
    /// Same session_id only.
    CurrentEpisode,
    /// Same plan / mission / task, falling back to session.
    CurrentMission,
    /// Deliberate cross-episode retrieval. Foreign rows are admitted.
    ExplicitCrossEpisode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_never_matches() {
        let a = EpisodeContext::default();
        let b = EpisodeContext {
            plan_id: Some("p1".into()),
            session_id: Some("s1".into()),
            ..Default::default()
        };
        assert!(!a.same_session(&b));
        assert!(!a.same_mission(&b));
        assert!(!b.same_session(&a));
        assert!(!b.same_mission(&a));
    }

    #[test]
    fn same_session_requires_both_sides() {
        let a = EpisodeContext { session_id: Some("s1".into()), ..Default::default() };
        let b = EpisodeContext { session_id: Some("s2".into()), ..Default::default() };
        assert!(!a.same_session(&b));
    }

    #[test]
    fn same_mission_prefers_plan() {
        let a = EpisodeContext {
            plan_id: Some("pA".into()),
            session_id: Some("s1".into()),
            ..Default::default()
        };
        let b = EpisodeContext {
            plan_id: Some("pA".into()),
            session_id: Some("s9".into()),
            ..Default::default()
        };
        let c = EpisodeContext {
            plan_id: Some("pB".into()),
            session_id: Some("s1".into()),
            ..Default::default()
        };
        assert!(a.same_mission(&b));
        assert!(!a.same_mission(&c));
    }
}
