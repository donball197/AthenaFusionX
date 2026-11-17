use crate::evidence_markers::{has_mutation_marker, has_recon_marker};

#[derive(Debug, Clone, PartialEq)]
pub enum ContractStatus {
    Satisfied,
    RejectedMutation,
    PartialEvidence,
    NoEvidence,
}

pub struct ContractEvaluator;

impl ContractEvaluator {
    pub fn evaluate(
        contract: &crate::completion_engine::SubgoalContract,
        local_evidence: &str,
        current_belief: f32,
    ) -> ContractStatus {
        let missing: Vec<&str> = contract
            .required_artifact_paths
            .iter()
            .map(|a| a.path.as_str())
            .filter(|p| !crate::evidence_markers::has_artifact_write(local_evidence, p))
            .collect();

        // REGRESSION FIX (missing-artifact early return): missing artifacts
        // must NOT unconditionally classify NoEvidence. They guarantee
        // contract_satisfied == false below, so the flow reaches the final
        // block and correctly returns PartialEvidence when mutation or recon
        // markers are present.
        let has_mutation = has_mutation_marker(local_evidence);
        let has_recon = has_recon_marker(local_evidence);

        // IMPORTANT: An artifact write proves only that the mutation/write action
        // succeeded. It does NOT prove that the enclosing objective is satisfied.
        // Compound objectives must continue through their full evidence contract.
        if contract.is_mutation_task && !has_mutation {
            return if has_recon {
                ContractStatus::PartialEvidence
            } else {
                ContractStatus::NoEvidence
            };
        }

        // The typed completion contract is the sole authority for
        // mission satisfaction. Mutation/write markers and belief are
        // evidence, not proof of the enclosing objective.
        // Missing artifacts can never satisfy the contract, but must not
        // short-circuit the classification.
        let contract_satisfied = missing.is_empty()
            && crate::completion_engine::SubgoalCompletionEngine::evaluate(
                contract,
                local_evidence,
                current_belief,
            );

        if contract_satisfied {
            ContractStatus::Satisfied
        } else if has_mutation || has_recon {
            ContractStatus::PartialEvidence
        } else {
            ContractStatus::NoEvidence
        }
    }
}

/// BUG-19: deterministic contract-progress predicate used by the
/// critic gate. True iff at least one proposed action would satisfy
/// a currently-UNMET completion condition. No LLM involvement — the
/// question "does this proposal advance the contract?" has a
/// deterministic answer, and the LLM critic answered it
/// non-deterministically (task 2: identical 1-of-6 proposals were
/// approved on turns 13/24/28 and vetoed on 19/45/56).
pub(crate) fn proposal_advances_contract(
    proposal: &[crate::agent_protocol::AgentAction],
    contract: &crate::completion_engine::SubgoalContract,
    local_evidence: &str,
) -> bool {
    proposal.iter().any(|action| {
        contract.conditions.iter().any(|cond| {
            !crate::cortical::contract_advance::condition_is_met(cond, local_evidence)
                && action_would_satisfy(action, cond)
        })
    })
}

fn action_would_satisfy(
    action: &crate::agent_protocol::AgentAction,
    cond: &crate::completion_engine::CompletionCondition,
) -> bool {
    use crate::agent_protocol::AgentAction as A;
    use crate::completion_engine::CompletionCondition as C;
    match (action, cond) {
        // A SearchCode emits `[SEARCH_CODE_OK] Path: <p> | Query: '<q>'`,
        // which search_checked_satisfied() accepts as proof that
        // SearchCheckedAcquired{target:"<leaf>:<term>"} was checked.
        (A::SearchCode { .. }, C::SearchCheckedAcquired { target }) => {
            crate::cortical::contract_advance::search_action_matches_target(
                action, target,
            )
        }
        // Path-bound reads emit [READ_OK] / [READ_FUNCTION_OK] which
        // satisfies ReadPathAcquired{path}.
        (A::ReadFile { path, .. }, C::ReadPathAcquired { path: want })
        | (A::ReadFunction { path, .. }, C::ReadPathAcquired { path: want }) => {
            crate::cortical::contract_advance::same_workspace_path(path, want)
        }
        // BUG-19 gap exposed by the task_1 diagnostic: write-producing
        // actions satisfy write contracts, but the previous match had no
        // arms for them. A `write_file { audit/cargo_check.txt }` proposal
        // that would resolve `WritePathAcquired{audit/cargo_check.txt}`
        // fell through to `_ => false`, was vetoed by the LLM critic on
        // subjective judgment, and the node spun out on repeat-vetoes
        // until belief exhausted at 0.02.

        // Any write action satisfies a bare write marker.
        (A::WriteFile { .. }, C::WriteMarkerAcquired)
        | (A::DiffPatch { .. }, C::WriteMarkerAcquired)
        | (A::PatchFunction { .. }, C::WriteMarkerAcquired)
        | (A::PatchFile { .. }, C::WriteMarkerAcquired)
        | (A::ReplaceLines { .. }, C::WriteMarkerAcquired)
        | (A::AtomicPatch { .. }, C::WriteMarkerAcquired)
        | (A::SynthesizeReport { .. }, C::WriteMarkerAcquired) => true,

        // Path-bound writes: match path or a suffix of it (allows the
        // contract's relative form and the executor's absolute form to
        // reconcile, same tolerance as ReadPathAcquired above).
        (A::WriteFile { path, .. }, C::WritePathAcquired { path: want })
        | (A::DiffPatch { path, .. }, C::WritePathAcquired { path: want })
        | (A::PatchFunction { path, .. }, C::WritePathAcquired { path: want })
        | (A::PatchFile { path, .. }, C::WritePathAcquired { path: want })
        | (A::ReplaceLines { path, .. }, C::WritePathAcquired { path: want }) => {
            crate::cortical::contract_advance::same_workspace_path(path, want)
        }
        (A::SynthesizeReport { output_path, .. }, C::WritePathAcquired { path: want }) => {
            crate::cortical::contract_advance::same_workspace_path(output_path, want)
        }
        (A::AtomicPatch { patches, .. }, C::WritePathAcquired { path: want }) => {
            patches.iter().any(|patch| {
                crate::cortical::contract_advance::same_workspace_path(
                    &patch.path, want,
                )
            })
        }

        // Any ExecuteCommand satisfies a bare command marker. The
        // stricter exact-command bypass at cortical_loop/critic.rs (REQUIRED-COMMAND BYPASS) fires earlier
        // when the contract is `CommandExecutionVerified` — do not
        // duplicate that logic here.
        (A::ExecuteCommand { .. }, C::CommandMarkerAcquired) => true,

        _ => false,
    }
}


#[cfg(test)]
mod proposal_path_matching_tests {
    use super::*;
    use crate::agent_protocol::{AgentAction, FilePatch};
    use crate::completion_engine::CompletionCondition;

    #[test]
    fn atomic_patch_matches_a_path_in_its_patch_list() {
        let action = AgentAction::AtomicPatch {
            patches: vec![
                FilePatch {
                    path: "crates/runtime_core/src/first.rs".into(),
                    search: "old".into(),
                    replace: "new".into(),
                },
                FilePatch {
                    path: "crates/runtime_core/src/target.rs".into(),
                    search: "before".into(),
                    replace: "after".into(),
                },
            ],
            reasoning: "test".into(),
            _thought_process: None,
        };

        let condition = CompletionCondition::WritePathAcquired {
            path: "crates/runtime_core/src/target.rs".into(),
        };

        assert!(action_would_satisfy(&action, &condition));
    }

    #[test]
    fn atomic_patch_rejects_unrelated_same_filename_path() {
        let action = AgentAction::AtomicPatch {
            patches: vec![FilePatch {
                path: "crates/runtime_core/src/target.rs".into(),
                search: "old".into(),
                replace: "new".into(),
            }],
            reasoning: "test".into(),
            _thought_process: None,
        };

        let condition = CompletionCondition::WritePathAcquired {
            path: "bins/other/src/target.rs".into(),
        };

        assert!(!action_would_satisfy(&action, &condition));
    }

    #[test]
    fn atomic_patch_with_no_patches_does_not_match_path_condition() {
        let action = AgentAction::AtomicPatch {
            patches: vec![],
            reasoning: "test".into(),
            _thought_process: None,
        };

        let condition = CompletionCondition::WritePathAcquired {
            path: "crates/runtime_core/src/target.rs".into(),
        };

        assert!(!action_would_satisfy(&action, &condition));
    }
}

#[cfg(test)]
mod p1b_search_checked_tests {
    use super::*;
    use crate::agent_protocol::AgentAction;
    use crate::completion_engine::CompletionCondition;

    #[test]
    fn full_path_checked_target_is_advanced_by_matching_search() {
        let action = AgentAction::SearchCode {
            _thought_process: None,
            path: "crates/runtime_core/src".into(),
            query: "worker".into(),
            reasoning: String::new(),
        };
        let cond = CompletionCondition::SearchCheckedAcquired {
            target: "crates/runtime_core/src:worker".into(),
        };
        assert!(action_would_satisfy(&action, &cond));
    }
}
