//! Iterative compile-remediation state machine.
//!
//! Standalone, dependency-free. The cortical loop owns one instance
//! per `EpistemicSession` and drives it from the package-scoped
//! compiler-critic branch. Nothing here touches the graph, the
//! executor, or the filesystem; the state machine classifies a
//! compile result and the caller decides what to do with the
//! classification.
//!
//! Design contract (from the review):
//!
//! * **Session-internal.** Remediation state is control-flow state,
//!   not externally meaningful outcome. It is not propagated through
//!   `CortexOutcome`. Observability is provided via evidence markers.
//!
//! * **Silent retries.** A compiler-corrective retry does not weaken
//!   the active hypothesis. The compiler diagnostic is the corrective
//!   signal; applying additional belief decay would conflate "the
//!   proposed mutation failed to compile" with "the overall hypothesis
//!   is becoming less credible." Only budget exhaustion triggers the
//!   full failure penalty, and that is handled by the caller, not
//!   here.
//!
//! * **Explicit state, not inferred.** The retry counter is a field,
//!   never derived by scanning `local_evidence` for marker strings.
//!   This makes the fourth-failure boundary deterministic regardless
//!   of evidence truncation, ordering, or history.
//!
//! * **Episode boundary is the physical source state.** An episode
//!   begins when a mutation leaves the source broken; it ends when the
//!   compiler succeeds or the caller explicitly resets after a
//!   confirmed revert. It is not keyed on the hypothesis, the mutation
//!   path, or the node.
//!
//! * **Caller-enforced invariant.** This module does not know which
//!   mutation an active episode belongs to, and it does not attempt
//!   to. The Layer 1 caller must enforce: while
//!   `CompileRemediationState` is active, only the corrective
//!   package-scoped compiler verification for that retained mutation
//!   may advance the state. An unrelated compiler check must not
//!   call `classify`. That discipline is a property of the call
//!   site, not of this module.

/// Retained corrective iterations permitted before the compiler
/// critic reverts the failed mutation. Failures 1..=BUDGET are
/// retained for retry; failure BUDGET+1 triggers rollback.
pub const COMPILE_RETRY_BUDGET: u32 = 3;

/// Live remediation state. Session-internal; not part of
/// `CortexOutcome`.
///
/// The state machine itself is driven entirely by two fields:
///
/// * `source_is_broken` — the episode boundary
/// * `iterations_since_breakage` — the retry counter
///
/// `episode_label` is advisory metadata for evidence and logging only.
/// It does not affect boundary detection or iteration counting.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileRemediationState {
    source_is_broken: bool,
    iterations_since_breakage: u32,
    episode_label: Option<String>,
}

// Manual impl is intentional and behaviorally identical to #[derive(Default)];
// this file is contract-frozen and its 15 unit tests pin the initial state.
// Kept as an explicit impl so future field additions are forced to declare
// their initial value rather than silently inheriting Default.
#[allow(clippy::derivable_impls)]
impl Default for CompileRemediationState {
    fn default() -> Self {
        Self { source_is_broken: false, iterations_since_breakage: 0, episode_label: None }
    }
}

/// Classification of a single compiler-critic outcome.
///
/// No belief decay is exposed. The policy is that remediation retries
/// are silent on the hypothesis; only the caller's budget-exhaustion
/// path applies the full failure penalty.
#[derive(Debug, Clone, PartialEq)]
pub enum CompileDecision {
    /// Compiler exited zero. Clears remediation state. The caller
    /// emits `[COMPILE_OK]` through the existing success branch.
    Success,

    /// Compiler failed, and the retry budget has room. Caller keeps
    /// the broken source on disk, appends the diagnostic to evidence,
    /// and returns to `Strategize`. No belief change.
    RetainForRetry { iteration: u32 },

    /// Compiler failed and the retry budget is exhausted. Caller
    /// reverts the mutation via `git checkout`, applies the full
    /// failure penalty, then calls `reset()` on the remediation state
    /// after confirming the revert succeeded.
    BudgetExhausted { total_iterations: u32 },
}

impl CompileRemediationState {
    /// Classify a compile result and advance the state.
    ///
    /// * `exit_success == true` → clears state, returns `Success`.
    /// * `exit_success == false`, source not yet known broken →
    ///   starts a fresh episode at iteration 1.
    /// * `exit_success == false`, source already broken → advances
    ///   the iteration counter.
    ///
    /// INVARIANT (caller-enforced): while this state is active, only
    /// the corrective package-scoped compiler verification for the
    /// retained mutation may call this method. An unrelated compiler
    /// check must not advance the episode.
    pub fn classify(&mut self, exit_success: bool) -> CompileDecision {
        if exit_success {
            self.source_is_broken = false;
            self.iterations_since_breakage = 0;
            self.episode_label = None;
            return CompileDecision::Success;
        }

        if !self.source_is_broken {
            self.source_is_broken = true;
            self.iterations_since_breakage = 1;
        } else {
            self.iterations_since_breakage = self.iterations_since_breakage.saturating_add(1);
        }

        if self.iterations_since_breakage > COMPILE_RETRY_BUDGET {
            CompileDecision::BudgetExhausted { total_iterations: self.iterations_since_breakage }
        } else {
            CompileDecision::RetainForRetry { iteration: self.iterations_since_breakage }
        }
    }

    /// Explicit reset. Idempotent. The caller invokes this after a
    /// confirmed revert. The state machine does not auto-reset on
    /// `BudgetExhausted` because a failed revert leaves the source
    /// broken and must remain in the same episode.
    pub fn reset(&mut self) {
        self.source_is_broken = false;
        self.iterations_since_breakage = 0;
        self.episode_label = None;
    }

    /// Advisory identity of the mutation or node under remediation.
    /// Set by the caller at the start of an episode. Does not affect
    /// boundary detection.
    pub fn set_episode_label(&mut self, label: impl Into<String>) {
        self.episode_label = Some(label.into());
    }

    /// Current episode label, if the caller set one.
    pub fn episode_label(&self) -> Option<&str> {
        self.episode_label.as_deref()
    }

    /// True while the source on disk is known to be in a
    /// compile-failing state that has not been resolved.
    pub fn is_active(&self) -> bool {
        self.source_is_broken
    }

    /// Iterations consumed so far in the current episode. Zero when
    /// the source is not broken.
    pub fn iterations(&self) -> u32 {
        self.iterations_since_breakage
    }
}

impl CompileDecision {
    /// True for `Success`. Convenience for the caller's branch.
    pub fn is_success(&self) -> bool {
        matches!(self, CompileDecision::Success)
    }

    /// True for `BudgetExhausted`. Convenience for the caller's
    /// branch.
    pub fn is_budget_exhausted(&self) -> bool {
        matches!(self, CompileDecision::BudgetExhausted { .. })
    }
}

/// Format the evidence string that the compiler critic appends to
/// `session.local_evidence` for a given decision.
///
/// `Success` returns `None` because the existing success path emits
/// `[COMPILE_OK]` through a separate code branch.
///
/// `RetainForRetry` and `BudgetExhausted` return `Some(evidence)`
/// with distinct markers so a verifier — or a human reading the log —
/// can tell a retained retry from a terminal exhaustion. The compiler
/// diagnostic is carried verbatim in both cases.
///
/// The caller may optionally prefix the returned string with the
/// episode label if the state has one; this function does not read
/// that state to keep it a pure function of its arguments.
pub fn format_compile_failure_evidence(
    decision: &CompileDecision,
    packages: &[String],
    err_preview: &str,
) -> Option<String> {
    match decision {
        CompileDecision::Success => None,

        CompileDecision::RetainForRetry { iteration } => Some(format!(
            "[COMPILE_FAIL_ITERATION {iteration}]\n\
             [COMPILE_FAIL] The source under mutation does not compile. \
             The compiler error below is authoritative. The source on disk \
             currently contains the failing patch.\n\
             [COMPILE_FAIL_ACTION] Use `read_function` on the target to see \
             the exact current source, then propose the smallest \
             `patch_function` action that fixes the compiler errors. Do NOT \
             revert. Do NOT reformat. Do NOT repeat the previously-failing \
             body verbatim. Fix only what the compiler reports.\n\
             [COMPILE_FAIL_DETAIL]\n{err_preview}\n"
        )),

        CompileDecision::BudgetExhausted { total_iterations } => Some(format!(
            "[COMPILE_FAIL_BUDGET_EXHAUSTED] {total_iterations} \
             compile-fail iterations without success.\n\
             [COMPILE_FAIL] Mutation failed package-scoped compiler check \
             for [{}]:\n{err_preview}",
            packages.join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_package() -> Vec<String> {
        vec!["math_utils".to_string()]
    }

    // ------------------------------------------------------------------
    // State transitions
    // ------------------------------------------------------------------

    #[test]
    fn default_state_is_inactive() {
        let s = CompileRemediationState::default();
        assert!(!s.is_active());
        assert_eq!(s.iterations(), 0);
        assert!(s.episode_label().is_none());
    }

    #[test]
    fn fresh_success_keeps_state_clean() {
        let mut s = CompileRemediationState::default();
        let d = s.classify(true);
        assert_eq!(d, CompileDecision::Success);
        assert!(d.is_success());
        assert!(!s.is_active());
        assert_eq!(s.iterations(), 0);
    }

    #[test]
    fn fresh_failure_starts_episode_at_iteration_one() {
        let mut s = CompileRemediationState::default();
        let d = s.classify(false);
        assert_eq!(d, CompileDecision::RetainForRetry { iteration: 1 });
        assert!(s.is_active());
        assert_eq!(s.iterations(), 1);
    }

    #[test]
    fn retry_chain_hits_budget_exactly_at_fourth_failure() {
        let mut s = CompileRemediationState::default();

        assert_eq!(s.classify(false), CompileDecision::RetainForRetry { iteration: 1 });
        assert_eq!(s.classify(false), CompileDecision::RetainForRetry { iteration: 2 });
        assert_eq!(s.classify(false), CompileDecision::RetainForRetry { iteration: 3 });

        let exhausted = s.classify(false);
        assert_eq!(exhausted, CompileDecision::BudgetExhausted { total_iterations: 4 });
        assert!(exhausted.is_budget_exhausted());
    }

    #[test]
    fn success_mid_chain_resets_episode() {
        let mut s = CompileRemediationState::default();

        assert_eq!(s.classify(false), CompileDecision::RetainForRetry { iteration: 1 });
        assert_eq!(s.classify(false), CompileDecision::RetainForRetry { iteration: 2 });
        assert_eq!(s.classify(true), CompileDecision::Success);
        assert!(!s.is_active());

        // A new failure after success starts a fresh episode.
        assert_eq!(s.classify(false), CompileDecision::RetainForRetry { iteration: 1 });
    }

    #[test]
    fn explicit_reset_clears_state() {
        let mut s = CompileRemediationState::default();
        s.classify(false);
        s.classify(false);
        assert!(s.is_active());
        assert_eq!(s.iterations(), 2);

        s.reset();
        assert!(!s.is_active());
        assert_eq!(s.iterations(), 0);

        // Post-reset failure is a new episode at iteration 1.
        assert_eq!(s.classify(false), CompileDecision::RetainForRetry { iteration: 1 });
    }

    #[test]
    fn budget_exhausted_does_not_auto_reset() {
        // Documents the contract: after BudgetExhausted the state
        // remains active until the caller explicitly resets. A
        // subsequent failure therefore stays in the same episode.
        let mut s = CompileRemediationState::default();
        for _ in 0..COMPILE_RETRY_BUDGET {
            s.classify(false);
        }
        assert_eq!(s.classify(false), CompileDecision::BudgetExhausted { total_iterations: 4 });
        assert!(s.is_active());

        // Another failure without resetting stays exhausted.
        assert_eq!(s.classify(false), CompileDecision::BudgetExhausted { total_iterations: 5 });
    }

    #[test]
    fn success_overrides_broken_state_regardless_of_history() {
        let mut s = CompileRemediationState::default();
        s.classify(false);
        s.classify(false);
        let d = s.classify(true);
        assert_eq!(d, CompileDecision::Success);
        assert!(!s.is_active());
    }

    #[test]
    fn iteration_counter_is_saturating() {
        // Defensive: even if a caller drives the state far past the
        // budget, the counter must not wrap.
        let mut s = CompileRemediationState::default();
        for _ in 0..10 {
            s.classify(false);
        }
        assert!(s.iterations() > COMPILE_RETRY_BUDGET);
        assert_ne!(s.iterations(), 0);
    }

    // ------------------------------------------------------------------
    // Episode label
    // ------------------------------------------------------------------

    #[test]
    fn episode_label_survives_retries_and_exhaustion() {
        let mut s = CompileRemediationState::default();
        s.set_episode_label("task_2:add");
        assert_eq!(s.episode_label(), Some("task_2:add"));

        s.classify(false);
        assert_eq!(s.episode_label(), Some("task_2:add"));

        s.classify(false);
        assert_eq!(s.episode_label(), Some("task_2:add"));

        s.classify(false);
        s.classify(false); // exhausted
        assert_eq!(s.episode_label(), Some("task_2:add"));
    }

    #[test]
    fn episode_label_cleared_on_success() {
        let mut s = CompileRemediationState::default();
        s.set_episode_label("task_2:add");
        s.classify(true);
        assert!(s.episode_label().is_none());
    }

    #[test]
    fn episode_label_cleared_on_reset() {
        let mut s = CompileRemediationState::default();
        s.set_episode_label("task_2:add");
        s.classify(false);
        s.reset();
        assert!(s.episode_label().is_none());
    }

    // ------------------------------------------------------------------
    // Evidence format
    // ------------------------------------------------------------------

    #[test]
    fn success_produces_no_failure_evidence() {
        let ev = format_compile_failure_evidence(
            &CompileDecision::Success,
            &one_package(),
            "some error",
        );
        assert!(ev.is_none());
    }

    #[test]
    fn retry_evidence_carries_iteration_marker_and_error() {
        let err = "error[E0425]: cannot find value `missing_constant` in this scope";
        let ev = format_compile_failure_evidence(
            &CompileDecision::RetainForRetry { iteration: 2 },
            &one_package(),
            err,
        )
        .expect("RetainForRetry must produce evidence");

        assert!(ev.contains("[COMPILE_FAIL_ITERATION 2]"), "missing iteration marker");
        assert!(ev.contains("[COMPILE_FAIL]"), "missing failure marker");
        assert!(ev.contains("[COMPILE_FAIL_ACTION]"), "missing action marker");
        assert!(ev.contains("[COMPILE_FAIL_DETAIL]"), "missing detail marker");
        assert!(ev.contains(err), "compiler diagnostic not carried verbatim");
        assert!(
            !ev.contains("[COMPILE_FAIL_BUDGET_EXHAUSTED]"),
            "retry evidence must not contain budget-exhausted marker"
        );
    }

    #[test]
    fn budget_exhausted_evidence_carries_total_and_packages() {
        let err = "error[E0425]: cannot find value `missing_constant` in this scope";
        let pkgs = vec!["math_utils".to_string(), "runtime_core".to_string()];
        let ev = format_compile_failure_evidence(
            &CompileDecision::BudgetExhausted { total_iterations: 4 },
            &pkgs,
            err,
        )
        .expect("BudgetExhausted must produce evidence");

        assert!(ev.contains("[COMPILE_FAIL_BUDGET_EXHAUSTED]"), "missing exhausted marker");
        assert!(ev.contains("4"), "missing total iteration count");
        assert!(ev.contains("math_utils"), "missing package name");
        assert!(ev.contains("runtime_core"), "missing second package name");
        assert!(ev.contains(err), "compiler diagnostic not carried verbatim");
        assert!(
            !ev.contains("[COMPILE_FAIL_ITERATION"),
            "exhausted evidence must not masquerade as a retained retry"
        );
    }
}
