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
        let mut condition_satisfied_seen = false;
        let mut refused_cmds: Vec<String> = Vec::new();

        for action in actions {
            let action_key = attempt_key(session, &action);
            let attempt_count = session.global_action_attempt_count(&action_key);

            // REFUSED-ACTION VETO: the read-only gate refuses before an action
            // ever counts as attempted, so classify_duplicate never sees it and
            // the same refused proposal was re-admitted every turn (60-turn
            // spin). Once a refusal marker exists, veto repeats of any action
            // the gate would refuse again.
            if session.contract.read_only
                && crate::cortical::read_only_gate::violates_read_only(&action)
                && session.local_evidence.contains("[READ_ONLY_VIOLATION]")
            {
                self.log(format!(
                    "\x1b[1;31m>> [ANTI-LOOP] Action [{}] was already refused by the read-only gate; vetoing re-proposal.\x1b[0m",
                    action_key
                ));
                refused_cmds.push(match &action {
                    AgentAction::ExecuteCommand { command, .. } => command.clone(),
                    _ => action_key.clone(),
                });
                vetoed_keys.push(action_key);
                continue;
            }

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
            let write_target_absent = is_retryable_absent_write(&action, attempt_count);

            if attempt_count >= 1 && !bypass_antiloop_for_retention && !write_target_absent {
                match crate::anti_loop::classify_duplicate(
                    &action,
                    &session.contract,
                    &session.local_evidence,
                ) {
                    crate::anti_loop::FilterOutcome::ConditionSatisfied => {
                        self.log(
                            "\x1b[1;32m>> [ANTI-LOOP RESOLVED] Required command already executed; condition satisfied. Skipping redispatch.\x1b[0m"
                                .to_string(),
                        );
                        condition_satisfied_seen = true;
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

        // REFUSAL -> HALT: when every proposed action was refused by the
        // read-only gate, report the refusal as a halt finding (once per node)
        // instead of spinning until belief decays.
        if filtered_actions.is_empty()
            && !refused_cmds.is_empty()
            && !session.local_evidence.contains("[REFUSAL_HALT_EMITTED]")
        {
            let cmds: Vec<String> =
                refused_cmds.iter().map(|c| c.replace('`', "")).collect();
            let report = format!(
                "Read-only gate refusal finding: the read-only command gate refused the required command before execution, so no command output was collected and this audit criterion is not met. Refused command: {}. The gate refuses command substitution, redirection and other constructs it cannot prove read-only; the command must be rewritten without them to be retried.",
                cmds.join(" ; ")
            );
            session.local_evidence.push_str("[REFUSAL_HALT_EMITTED]\n");
            self.log(
                "\x1b[1;36m>> [ANTI-LOOP] Converting read-only refusal into a halt finding.\x1b[0m"
                    .to_string(),
            );
            return AntiLoopOutcome::Proceed(vec![AgentAction::Halt {
                _thought_process: None,
                reasoning: report,
            }]);
        }

        if filtered_actions.is_empty() && condition_satisfied_seen && vetoed_keys.is_empty() {
            self.log(
                "[ANTI-LOOP RESOLVED] All proposed actions already satisfy their required command condition; returning to Strategize for any remaining verification or remediation."
                    .to_string(),
            );
            return AntiLoopOutcome::NeedsStrategize;
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
            let stuck_count = consecutive_loop_vetoes(&session.local_evidence);

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

/// Actions whose result depends on the current state of the workspace. The same
/// command / read / search is NOT a duplicate once a mutation has landed in
/// between (re-running a failing command after patching its target; re-reading a
/// file after editing it).
fn is_state_sensitive(action: &AgentAction) -> bool {
    matches!(
        action,
        AgentAction::ExecuteCommand { _thought_process: None, .. }
            | AgentAction::ReadFile { _thought_process: None, .. }
            | AgentAction::ReadFileChunk { _thought_process: None, .. }
            | AgentAction::ReadFunction { _thought_process: None, .. }
            | AgentAction::SearchCode { _thought_process: None, .. }
    )
}

/// Number of physical mutation markers recorded so far in the session evidence.
pub(crate) fn mutation_epoch(evidence: &str) -> usize {
    evidence
        .lines()
        .filter(|l| {
            l.trim_start()
                .strip_prefix('[')
                .and_then(|r| r.split(']').next())
                .map(|m| {
                    matches!(
                        m,
                        "WRITE_OK" | "PATCH_FUNCTION_OK" | "DIFF_PATCH_OK" | "REPLACE_LINES_OK"
                    ) || m.starts_with("ATOMIC_PATCH")
                        || m.starts_with("SYNTHESIZE")
                })
                .unwrap_or(false)
        })
        .count()
}

/// Anti-loop attempt key: the canonical action key, plus the mutation epoch for
/// state-sensitive actions so a repeat after a real change is a new attempt.
pub(crate) fn attempt_key_for(action: &AgentAction, evidence: &str) -> String {
    let base = crate::cortical::action_identity::action_key(action);
    if is_state_sensitive(action) {
        format!("{}@m{}", base, mutation_epoch(evidence))
    } else {
        base
    }
}

pub(crate) fn attempt_key(session: &EpistemicSession, action: &AgentAction) -> String {
    attempt_key_for(action, &session.local_evidence)
}

/// A repeated WriteFile is not a duplicate if its previous effect is gone
/// (reverted, or never landed): the target is physically absent. Capped so an
/// identical write that keeps failing still trips the veto.
fn is_retryable_absent_write(action: &AgentAction, attempt_count: u32) -> bool {
    attempt_count <= 2
        && matches!(
            action,
            AgentAction::WriteFile { _thought_process: None, path, .. }
                if !std::path::Path::new(path.trim()).exists()
        )
}

/// Number of all-duplicate LOOP_VETO lines since the last physical success
/// marker (`[..._OK]`). An interleaved success means the node made progress,
/// so the stuck budget measures consecutive stalls, not lifetime vetoes.
fn consecutive_loop_vetoes(evidence: &str) -> usize {
    let mut count = 0usize;
    for l in evidence.lines().rev() {
        if l.starts_with("[LOOP_VETO] All proposed actions already attempted") {
            count += 1;
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
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    const VETO: &str =
        "[LOOP_VETO] All proposed actions already attempted: k. Must choose different action(s).";

    #[test]
    fn empty_evidence_is_zero() {
        assert_eq!(consecutive_loop_vetoes(""), 0);
    }

    #[test]
    fn counts_trailing_vetoes() {
        let ev = format!("{VETO}\n{VETO}\n{VETO}\n");
        assert_eq!(consecutive_loop_vetoes(&ev), 3);
    }

    #[test]
    fn success_marker_resets_the_budget() {
        let ev = format!("{VETO}\n{VETO}\n[WRITE_OK] path=a.rs\n{VETO}\n");
        assert_eq!(consecutive_loop_vetoes(&ev), 1);
    }

    #[test]
    fn six_vetoes_interleaved_with_progress_do_not_trip_the_budget() {
        let mut ev = String::new();
        for _ in 0..6 {
            ev.push_str(VETO);
            ev.push('\n');
            ev.push_str("[READ_OK] path=a.rs\n");
        }
        assert!(consecutive_loop_vetoes(&ev) < 6);
    }

    #[test]
    fn six_consecutive_vetoes_do_trip_the_budget() {
        let mut ev = String::from("[COMPILE_OK] fine\n");
        for _ in 0..6 {
            ev.push_str(VETO);
            ev.push('\n');
        }
        assert_eq!(consecutive_loop_vetoes(&ev), 6);
    }

    #[test]
    fn non_success_markers_do_not_reset() {
        let ev =
            format!("{VETO}\n[CRITIC_VETO] x\n[STRATEGY_VIOLATION] y\n[HALT_REJECTED] z\n{VETO}\n");
        assert_eq!(consecutive_loop_vetoes(&ev), 2);
    }

    fn write_action(path: &str) -> AgentAction {
        serde_json::from_value(
            serde_json::json!({"type": "write_file", "path": path, "content": "x"}),
        )
        .expect("write_file JSON must deserialize as AgentAction")
    }

    #[test]
    fn absent_write_target_is_retryable_up_to_the_cap() {
        let a = write_action("/nonexistent_antiloop_dir/nope.rs");
        assert!(is_retryable_absent_write(&a, 1));
        assert!(is_retryable_absent_write(&a, 2));
        assert!(!is_retryable_absent_write(&a, 3));
    }

    #[test]
    fn existing_write_target_is_a_real_duplicate() {
        let p = std::env::temp_dir().join("antiloop_existing_target_test.txt");
        std::fs::write(&p, "x").unwrap();
        let a = write_action(&p.to_string_lossy());
        assert!(!is_retryable_absent_write(&a, 1));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn non_write_actions_never_bypass() {
        let a: AgentAction = serde_json::from_value(
            serde_json::json!({"type": "read_file", "path": "/nonexistent_antiloop_dir/x.rs"}),
        )
        .expect("read_file JSON must deserialize as AgentAction");
        assert!(!is_retryable_absent_write(&a, 1));
    }
}

#[cfg(test)]
mod attempt_key_tests {
    use super::*;

    fn cmd(c: &str) -> AgentAction {
        serde_json::from_value(serde_json::json!({"type": "execute_command", "command": c}))
            .expect("execute_command JSON must deserialize as AgentAction")
    }

    #[test]
    fn same_command_gets_a_new_key_after_a_mutation() {
        let a = cmd("python3 -c 'print(1)'");
        let before = "[ACTION FAILED] exit 1\n";
        let after = "[ACTION FAILED] exit 1\n[PATCH_FUNCTION_OK] m.py:divide\n";
        assert_eq!(attempt_key_for(&a, before), attempt_key_for(&a, before));
        assert_ne!(attempt_key_for(&a, before), attempt_key_for(&a, after));
    }

    #[test]
    fn write_actions_ignore_the_epoch() {
        let w: AgentAction = serde_json::from_value(
            serde_json::json!({"type": "write_file", "path": "a.txt", "content": "x"}),
        )
        .expect("write_file JSON must deserialize as AgentAction");
        assert_eq!(attempt_key_for(&w, ""), attempt_key_for(&w, "[WRITE_OK] a.txt\n"));
    }

    #[test]
    fn epoch_counts_only_mutation_markers() {
        let ev = "[CMD_OK] x\n[WRITE_OK] a\n[READ_OK] b\n[PATCH_FUNCTION_OK] c\n[WRITE_NOOP] d\n";
        assert_eq!(mutation_epoch(ev), 2);
        assert_eq!(mutation_epoch(""), 0);
    }
}
