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
        let has_mutation = has_mutation_marker(local_evidence);
        let has_recon = has_recon_marker(local_evidence);

        if (is_mutation_task || is_write_task) && !has_mutation {
            return if has_recon {
                ContractStatus::PartialEvidence
            } else {
                ContractStatus::NoEvidence
            };
        }

        if Self::is_compound_objective_satisfied(mission, local_evidence) {
            if current_belief >= 0.95 {
                return ContractStatus::Satisfied;
            }
            return ContractStatus::PartialEvidence;
        }

        if has_mutation || has_recon {
            return if current_belief >= 0.95 {
                ContractStatus::Satisfied
            } else {
                ContractStatus::PartialEvidence
            };
        }

        ContractStatus::NoEvidence
    }

    fn is_compound_objective_satisfied(mission: &str, evidence: &str) -> bool {
        let mission_lower = mission.to_lowercase();
        let objective_part = mission_lower
            .lines()
            .find(|line| line.starts_with("objective:"))
            .unwrap_or(mission_lower.as_str());

        let evidence_lower = evidence.to_lowercase();

        let and_segments: Vec<&str> = objective_part
            .split(" and ")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if and_segments.is_empty() {
            return false;
        }

        and_segments.iter().all(|segment| {
            let or_parts: Vec<&str> = segment
                .split(" or ")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            if or_parts.len() <= 1 {
                segment
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .any(|w| evidence_lower.contains(w))
            } else {
                or_parts.iter().any(|part| {
                    part.split_whitespace()
                        .filter(|w| w.len() > 3)
                        .any(|w| evidence_lower.contains(w))
                })
            }
        })
    }
}
