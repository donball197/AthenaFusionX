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
        // ExecuteCommand is handled earlier by is_required_verification_command
        // in the existing bypass; do not duplicate here.
        _ => false,
    }
}
