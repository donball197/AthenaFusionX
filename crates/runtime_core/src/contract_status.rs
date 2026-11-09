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
        (A::SearchCode { path, query, .. }, C::SearchCheckedAcquired { target }) => {
            let leaf = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path);
            target == &format!("{}:{}", leaf, query)
        }
        // Path-bound reads emit [READ_OK] / [READ_FUNCTION_OK] which
        // satisfies ReadPathAcquired{path}.
        (A::ReadFile { path, .. }, C::ReadPathAcquired { path: want })
        | (A::ReadFunction { path, .. }, C::ReadPathAcquired { path: want }) => {
            path == want || path.ends_with(want.as_str())
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
            path == want || path.ends_with(want.as_str())
        }
        (A::SynthesizeReport { output_path, .. }, C::WritePathAcquired { path: want }) => {
            output_path == want || output_path.ends_with(want.as_str())
        }

        // Any ExecuteCommand satisfies a bare command marker. The
        // stricter exact-command bypass at critic.rs:154 fires earlier
        // when the contract is `CommandExecutionVerified` — do not
        // duplicate that logic here.
        (A::ExecuteCommand { .. }, C::CommandMarkerAcquired) => true,

        _ => false,
    }
}
