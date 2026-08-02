use crate::mutation_critic::MutationEvaluation;

pub struct MutationCriticAudit;

impl MutationCriticAudit {
    pub fn summarize(evaluation: &MutationEvaluation) -> String {
        format!(
            "Mutation accepted={} similarity={} confidence_delta={} reason={}",
            evaluation.accepted,
            evaluation.structural_similarity,
            evaluation.confidence_delta,
            evaluation.reason
        )
    }
}
