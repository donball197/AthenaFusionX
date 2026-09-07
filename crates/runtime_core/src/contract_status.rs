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
        is_mutation_task: bool,
        is_write_task: bool,
        local_evidence: &str,
        current_belief: f32,
        mission: &str,
    ) -> ContractStatus {
        let mission_contract =
            crate::completion_engine::SubgoalCompletionEngine::infer_contract("ad_hoc", mission);
        let missing: Vec<&str> = mission_contract
            .required_artifact_paths
            .iter()
            .map(|a| a.path.as_str())
            .filter(|p| !crate::evidence_markers::has_artifact_write(local_evidence, p))
            .collect();

        if !missing.is_empty() {
            return ContractStatus::NoEvidence;
        }

        let has_mutation = has_mutation_marker(local_evidence);
        let has_recon = has_recon_marker(local_evidence);

        // IMPORTANT: An artifact write proves only that the mutation/write action
        // succeeded. It does NOT prove that the enclosing objective is satisfied.
        // Compound objectives must continue through their full evidence contract.
        if (is_mutation_task || is_write_task) && !has_mutation {
            return if has_recon {
                ContractStatus::PartialEvidence
            } else {
                ContractStatus::NoEvidence
            };
        }

        // The typed completion contract is the sole authority for
        // mission satisfaction. Mutation/write markers and belief are
        // evidence, not proof of the enclosing objective.
        let contract_satisfied = crate::completion_engine::SubgoalCompletionEngine::evaluate(
            &mission_contract,
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
