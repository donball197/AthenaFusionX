#!/usr/bin/env bash
set -euo pipefail
cat << 'EOF' > crates/runtime_core/src/contract_status.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum ContractStatus {
    /// The contract is mathematically proven to be complete.
    Satisfied,
    /// A mutation was attempted, but the evaluator rejected it.
    RejectedMutation,
    /// The agent has gathered some evidence, but not enough to satisfy all conditions.
    PartialEvidence,
    /// No useful evidence has been gathered yet.
    NoEvidence,
}

pub struct ContractEvaluator;

impl ContractEvaluator {
    /// Evaluates the current evidence against the contract deterministically.
    /// This replaces the duplicated `has_prior_evidence` and `compound_satisfied` blocks.
    pub fn evaluate(
        is_mutation_task: bool,
        is_write_task: bool,
        local_evidence: &str,
        current_belief: f32,
        mission: &str,
    ) -> ContractStatus {
        let has_mutation_marker = local_evidence.lines().any(|l| {
            l.starts_with("[WRITE_OK]") || 
            l.starts_with("[PATCH_FUNCTION_OK]") || 
            l.starts_with("[REPLACE_LINES_OK]") ||
            l.starts_with("[DIFF_PATCH_OK]")
        });

        let has_recon_marker = local_evidence.lines().any(|l| {
            l.starts_with("[READ_CHUNK_OK]") || 
            l.starts_with("[SEARCH_CODE_OK]") || 
            l.starts_with("[READ_OK]") || 
            l.starts_with("[CMD_OK]")
        });

        // 1. If it's a write/mutation task, we MUST have a success marker.
        if (is_mutation_task || is_write_task) && !has_mutation_marker {
            return if has_recon_marker { 
                ContractStatus::PartialEvidence 
            } else { 
                ContractStatus::NoEvidence 
            };
        }

        // 2. Compound objective check.
        if Self::is_compound_objective_satisfied(mission, local_evidence) {
            if current_belief >= 0.95 {
                return ContractStatus::Satisfied;
            }
            return ContractStatus::PartialEvidence;
        }

        // 3. Simple objective satisfied
        if has_mutation_marker || has_recon_marker {
            return if current_belief >= 0.95 {
                ContractStatus::Satisfied
            } else {
                ContractStatus::PartialEvidence
            };
        }

        ContractStatus::NoEvidence
    }

    /// Extracts the compound logic to keep the main evaluate function clean.
    /// REFACTOR NOTE: We still check the mission text, but we isolate this logic 
    /// into a single, testable function rather than duplicating it inline.
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
                segment.split_whitespace()
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
EOF
echo "wrote crates/runtime_core/src/contract_status.rs"
wc -l crates/runtime_core/src/contract_status.rs
