//! Contract-advance checks extracted from cortical_loop.rs (no `self`, no state).

use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;

/// Returns true if executing `action` would produce evidence that
/// satisfies a currently-unmet condition in the session contract.
///
/// Multi-criteria safety net: when the planner packs several
/// criteria into one node, the cortex's single-strategy model
/// cannot satisfy them sequentially -- after the first criterion
/// is met, the active strategy never transitions to the next, so
/// every action for the remaining criteria is rejected. Accepting
/// any action that advances an unmet criterion lets the model
/// progress through the criteria in any order.
///
/// The DAG verifier independently requires each criterion's marker
/// in evidence, so this relaxation cannot authorize completion
/// without the required physical evidence.
pub(crate) fn action_advances_unmet_contract_condition(
    action: &AgentAction,
    session: &EpistemicSession,
    mission: &str,
) -> bool {
    use crate::completion_engine::CompletionCondition as CC;

    for cond in &session.contract.conditions {
        if condition_is_met(cond, &session.local_evidence) {
            continue;
        }
        let advances = match cond {
            CC::ReadMarkerAcquired => {
                match crate::cortical::util::extract_named_target(mission, "READ_OK:") {
                    Some(target) => {
                        matches!(
                            action,
                            AgentAction::ReadFile { path, .. } if path.trim() == target
                        ) || matches!(
                            action,
                            AgentAction::ReadFileChunk { path, .. } if path.trim() == target
                        )
                    }
                    None => matches!(
                        action,
                        AgentAction::ReadFile { .. } | AgentAction::ReadFileChunk { .. }
                    ),
                }
            }
            CC::ReadPathAcquired { path: target } => {
                matches!(
                    action,
                    AgentAction::ReadFile { path, .. } if path.trim() == target
                ) || matches!(
                    action,
                    AgentAction::ReadFileChunk { path, .. } if path.trim() == target
                )
            }
            CC::SearchMarkerAcquired => matches!(action, AgentAction::SearchCode { .. }),
            CC::SearchTermAcquired { term } => {
                let expected = term.trim().to_lowercase();
                matches!(
                    action,
                    AgentAction::SearchCode { query, .. }
                        if query.trim().to_lowercase().contains(&expected)
                            || expected.contains(&query.trim().to_lowercase())
                )
            }
            CC::SearchMemoryMarkerAcquired => {
                matches!(action, AgentAction::SearchMemory { .. })
            }
            CC::WriteMarkerAcquired => {
                match crate::cortical::util::extract_named_target(mission, "WRITE_OK:") {
                    Some(target) => {
                        matches!(
                            action,
                            AgentAction::WriteFile { path, .. } if path.trim() == target
                        ) || matches!(
                            action,
                            AgentAction::SynthesizeReport { output_path, .. }
                                if output_path.trim() == target
                        )
                    }
                    None => matches!(
                        action,
                        AgentAction::WriteFile { .. } | AgentAction::SynthesizeReport { .. }
                    ),
                }
            }
            CC::WritePathAcquired { path: target } => {
                matches!(
                    action,
                    AgentAction::WriteFile { path, .. } if path.trim() == target
                ) || matches!(
                    action,
                    AgentAction::SynthesizeReport { output_path, .. }
                        if output_path.trim() == target
                )
            }
            CC::PatchMarkerAcquired => {
                match crate::cortical::util::extract_named_target(mission, "PATCH_FUNCTION_OK:") {
                    Some(target) => {
                        let mut parts = target.splitn(2, ':');
                        let expected_path = parts.next().unwrap_or("").trim();
                        let expected_fn = parts.next().unwrap_or("").trim();
                        if expected_path.is_empty() || expected_fn.is_empty() {
                            matches!(
                                action,
                                AgentAction::PatchFunction { .. }
                                    | AgentAction::DiffPatch { .. }
                                    | AgentAction::ReplaceLines { .. }
                            )
                        } else {
                            matches!(
                                action,
                                AgentAction::PatchFunction { path, function_name, .. }
                                    if path.trim() == expected_path
                                        && function_name.trim() == expected_fn
                            )
                        }
                    }
                    None => matches!(
                        action,
                        AgentAction::PatchFunction { .. }
                            | AgentAction::DiffPatch { .. }
                            | AgentAction::ReplaceLines { .. }
                    ),
                }
            }
            CC::CommandMarkerAcquired => {
                matches!(action, AgentAction::ExecuteCommand { .. })
            }
            CC::CommandExecutionVerified { .. } | CC::MathematicalExecutionVerified { .. } => {
                matches!(action, AgentAction::ExecuteCommand { .. })
            }
            CC::ReadFunctionMarkerAcquired { .. } => {
                match crate::cortical::util::extract_named_target(mission, "READ_FUNCTION_OK:") {
                    Some(target) => {
                        let mut parts = target.splitn(2, ':');
                        let expected_path = parts.next().unwrap_or("").trim();
                        let expected_fn = parts.next().unwrap_or("").trim();
                        if expected_path.is_empty() || expected_fn.is_empty() {
                            matches!(action, AgentAction::ReadFunction { .. })
                        } else {
                            matches!(
                                action,
                                AgentAction::ReadFunction { path, function_name, .. }
                                    if path.trim() == expected_path
                                        && function_name.trim() == expected_fn
                            )
                        }
                    }
                    None => matches!(action, AgentAction::ReadFunction { .. }),
                }
            }
            CC::SourceCodeEvidenceRequired => matches!(
                action,
                AgentAction::ReadFile { .. }
                    | AgentAction::ReadFileChunk { .. }
                    | AgentAction::SearchCode { .. }
            ),
            CC::HaltDelivered => matches!(action, AgentAction::Halt { .. }),
            CC::LineageEvidenceAcquired | CC::MinBeliefScore(_) => false,
        };
        if advances {
            return true;
        }
    }
    false
}

/// Probes the outcome verifier against a single condition to
/// determine whether it is already satisfied by the current
/// evidence. Belief is passed as 1.0 so a MinBeliefScore condition
/// would pass; MinBeliefScore is excluded from the match above
/// because no action advances it directly.
pub(crate) fn condition_is_met(
    cond: &crate::completion_engine::CompletionCondition,
    evidence: &str,
) -> bool {
    crate::outcome_verifier::OutcomeVerifier::mission_completion_authorized(
        evidence,
        1.0,
        std::slice::from_ref(cond),
    )
}
