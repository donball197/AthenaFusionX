//! Concrete predicates against the v1 local surface.
//!
//! Predicate::assess takes Option<&Evidence>.
//! Absence and irrelevance both map to NN inside assess
//! (this tree has no accepts/ignores gating layer).

use crate::assessment::Assessment;
use crate::evidence::{Evidence, Scope, TargetId};
use crate::predicate::Predicate;

/// Path-scoped read/observation established.
///
/// - None → NN
/// - target/scope missing or mismatched → NN (irrelevant)
/// - matching + success → SC
/// - matching + !success → RC
pub struct ReadPathEvidenceEstablished {
    pub target: TargetId,
    pub scope: Scope,
}

impl Predicate for ReadPathEvidenceEstablished {
    fn assess(&self, evidence: Option<&Evidence>) -> Assessment {
        let Some(e) = evidence else {
            return Assessment::NN;
        };

        let target_ok = e.target.as_ref() == Some(&self.target);
        let scope_ok = e.scope.as_ref() == Some(&self.scope);
        if !(target_ok && scope_ok) {
            return Assessment::NN;
        }

        if e.success {
            Assessment::SC
        } else {
            Assessment::RC
        }
    }
}
