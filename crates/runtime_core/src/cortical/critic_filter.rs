// crates/runtime_core/src/cortical/critic_filter.rs

use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;
use crate::cortical_loop::CorticalLoop;

pub(crate) enum AntiLoopOutcome {
    Proceed(Vec<AgentAction>),
    NeedsStrategize,
    StuckNodeFailed { belief: f32, evidence: String },
}

impl CorticalLoop {
    /// Filters duplicate actions, applies retention bypass during compile remediation,
    /// enforces anti-loop belief penalties, and trips the stuck-node budget.
    pub(crate) fn filter_anti_loop(
        &mut self,
        session: &mut EpistemicSession,
        actions: Vec<AgentAction>,
    ) -> AntiLoopOutcome {
        let mut filtered_actions: Vec<AgentAction> = Vec::with_capacity(actions.len());
        let mut vetoed_keys: Vec<String> = Vec::new();

        for action in actions {
            let action_key = crate::cortical::action_identity::action_key(&action);
            let attempt_count = session.global_action_attempt_count(&action_key);

            // STRICT ANTI-LOOP:
            // An identical action that has already crossed the execution
            // boundary is not new evidence. Drop it from the batch, but
            // let never-attempted sibling actions in the same proposal
            // proceed -- one stale action must not starve the rest.
            //
            // EXCEPTION — active compile remediation:
            // While a retention episode is open, the on-disk source is
            // known to have changed since the previously-recorded attempt.
            let bypass_antiloop_for_retention = session.compile_remediation.is_active()
                && matches!(
                    action,
                    AgentAction::PatchFunction { _thought_process: None, .. }
                        | AgentAction::ReadFunction { _thought_process: None, .. }
                        | AgentAction::ReadFileChunk { _thought_process: None, .. }
                        | AgentAction::ReadFile { _thought_process: None, .. }
                );

            // A repeated WriteFile is not a duplicate if its previous effect is
            // gone (reverted, or never landed): the target is physically absent.
            // Capped so an identical write that keeps failing still trips the veto.
            let write_target_absent = attempt_count <= 2
                && matches!(
                    &action,
                    AgentAction::WriteFile { _thought_process: None, path, .. }
                        if !std::path::Path::new(path.trim()).exists()
                );

            if attempt_count >= 1 && !bypass_antiloop_for_retention && !write_target_absent {
                match crate::anti_loop::classify_duplicate(
                    &action,
                    &session.contract,
                    &session.local_evidence,
                ) {
                    crate::anti_loop::FilterOutcome::ConditionSatisfied => {
                        self.log(
                            "\x1b[1;32m>> [ANTI-LOOP RESOLVED] Required command already executed; condition satisfied.\x1b[0m"
                                .to_string(),
                        );
                        filtered_actions.push(action);
                    }
                    crate::anti_loop::FilterOutcome::AllowRetry => {
                        self.log(
                            "\x1b[1;33m>> [ANTI-LOOP] Duplicate required by contract with no marker; permitting retry.\x1b[0m"
                                .to_string(),
                        );
                        filtered_actions.push(action);
                    }
                    crate::anti_loop::FilterOutcome::FilterAndDecay => {
                        self.log(format!(
                            "\x1b[1;31m>> [ANTI-LOOP] Duplicate action [{}] filtered from batch.\x1b[0m",
                            action_key
                        ));
                        vetoed_keys.push(action_key);
                    }
                }
            } else {
                if attempt_count >= 1 {
                    self.log(format!(
                        "\x1b[1;33m>> [ANTI-LOOP] Permitting repeated action [{}] (retention bypass or absent write target).\x1b[0m",
                        action_key
                    ));
                }
                filtered_actions.push(action);
            }
        }

        if filtered_actions.is_empty() {
            if let Some(h_id) = session.best_hypothesis_id {
                Self::decay_hypothesis_belief(session, h_id, 0.20);
                if let Some(node) = session.graph.nodes.get(&h_id) {
                    self.log(format!(
                        "\x1b[1;31m>> [ANTI-LOOP] Repeated stale proposal -- decaying hypothesis belief to {:.2}\x1b[0m",
                        node.belief_score
                    ));
                }
            }

            let loop_obs = format!(
                "[LOOP_VETO] All proposed actions already attempted: {}. \
                 Must choose different action(s).",
                vetoed_keys.join(", ")
            );
            session.local_evidence.push_str(&loop_obs);
            session.local_evidence.push('\n');

            const LOOP_VETO_BUDGET: usize = 6;
            // Count only vetoes since the last physical success marker: an
            // interleaved [..._OK] means the node made progress, so the budget
            // measures consecutive stalls, not lifetime vetoes.
            let mut stuck_count = 0usize;
            for l in session.local_evidence.lines().rev() {
                if l.starts_with("[LOOP_VETO] All proposed actions already attempted") {
                    stuck_count += 1;
                } else if l
                    .trim_start()
                    .strip_prefix('[')
                    .and_then(|r| r.split(']').next())
                    .map(|m| m.ends_with("_OK"))
                    .unwrap_or(false)
                {
                    break;
                }
            }

            if stuck_count >= LOOP_VETO_BUDGET {
                self.log(format!(
                    "\x1b[1;31m>> [STUCK NODE] {} all-duplicate proposal batches with no new evidence. Failing node early.\x1b[0m",
                    stuck_count
                ));
                session.local_evidence.push_str(
                    "[STUCK_NODE] Repeated duplicate proposals with no new evidence; the objective may be unachievable in this environment (missing dependency or unsupported platform).\n",
                );
                return AntiLoopOutcome::StuckNodeFailed {
                    belief: session.graph.best_hypothesis_belief(),
                    evidence: "Stuck: repeated duplicate proposals with no new evidence."
                        .to_string(),
                };
            }

            self.log(
                "[ANTI-LOOP RECOVERY] All actions in batch were duplicates; \
                 hypothesis belief decayed to force pivot. Re-entering Strategize."
                    .to_string(),
            );

            return AntiLoopOutcome::NeedsStrategize;
        }

        if !vetoed_keys.is_empty() {
            let loop_obs = format!(
                "[LOOP_VETO] Dropped already-attempted action(s) from batch: {}. \
                 Proceeding with remaining valid action(s).",
                vetoed_keys.join(", ")
            );
            session.local_evidence.push_str(&loop_obs);
            session.local_evidence.push('\n');
        }

        AntiLoopOutcome::Proceed(filtered_actions)
    }
}
