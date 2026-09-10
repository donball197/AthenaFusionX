//! Probabilistic Memory & Causal Learning Engine for AthenaFusionX.
//! Calculates P(Relevance | Task) and maintains Positive Genomes vs Negative Constraints.

use crate::memory_cortex::MemoryCortex;
use crate::sovereign_core::SovereignOutcome;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPrior {
    pub directive: String,
    pub pattern_type: String, // "POSITIVE_GENOME" | "NEGATIVE_CONSTRAINT"
    pub evidence: String,
    pub confidence: f32,
}

pub struct ProbabilisticMemory;

impl ProbabilisticMemory {
    /// Queries the VectorVault and returns probabilistic memory priors (both winning reflexes and negative constraints)
    pub async fn retrieve_priors(
        memory: &Arc<MemoryCortex>,
        api_key: &str,
        current_directive: &str,
    ) -> (Vec<MemoryPrior>, Vec<String>) {
        let mut positive_priors = Vec::new();
        let mut negative_constraints = Vec::new();

        // 1. Calculate FastEmbed vector for current directive
        if let Ok(query_vec) =
            nexus_substrate::vector_bridge::get_embedding(api_key, current_directive).await
        {
            if let Ok(memories) = memory.semantic_search(&query_vec, 3).await {
                for m in memories {
                    if m.result.contains("[HIGH_FITNESS_PATTERN]")
                        || m.result.contains("[COMPILE_OK]")
                    {
                        positive_priors.push(MemoryPrior {
                            directive: m.directive.clone(),
                            pattern_type: "POSITIVE_GENOME".to_string(),
                            evidence: m.result.lines().take(8).collect::<Vec<_>>().join("\n"),
                            confidence: 0.95,
                        });
                    } else if m.result.contains("[COMPILE_FAIL]")
                        || m.result.contains("[ACTION_GATE_BLOCKED]")
                        || m.result.contains("[MUTATION_REVERTED]")
                    {
                        negative_constraints.push(format!(
                            "NEGATIVE CONSTRAINT (Avoid on '{}'): {}",
                            m.directive,
                            m.result.lines().take(4).collect::<Vec<_>>().join(" ")
                        ));
                    }
                }
            }
        }

        (positive_priors, negative_constraints)
    }

    /// Automatically commits learning from completed missions into the durable memory palace
    pub async fn commit_learning(
        memory: &Arc<MemoryCortex>,
        api_key: &str,
        directive: &str,
        outcome: &SovereignOutcome,
    ) -> Result<()> {
        let embedding =
            nexus_substrate::vector_bridge::get_embedding(api_key, directive).await.ok();

        if outcome.is_successful && outcome.belief_score >= 0.85 {
            let genome_payload = format!(
                "[HIGH_FITNESS_PATTERN] Confidence: {:.2} | Turns: {}\nVERIFIED PROOF:\n{}",
                outcome.belief_score, outcome.turns_taken, outcome.verified_proof
            );

            memory
                .store_memory("SovereignGenome", directive, &genome_payload, "Internal", embedding)
                .await?;

            tracing::info!(
                "\x1b[1;32m>> [MEMORY EVOLUTION] Positive Genome strand committed to VectorVault for: {}\x1b[0m",
                directive
            );
        } else if !outcome.is_successful {
            let failure_payload = format!(
                "[NEGATIVE_CONSTRAINT] Mission failed to reach verification threshold ({:.2}).\nEVIDENCE:\n{}",
                outcome.belief_score, outcome.verified_proof
            );

            memory
                .store_memory(
                    "NegativeConstraint",
                    directive,
                    &failure_payload,
                    "Internal",
                    embedding,
                )
                .await?;

            tracing::warn!(
                "\x1b[1;33m>> [MEMORY EVOLUTION] Negative constraint indexed in VectorVault to inhibit future failure paths.\x1b[0m"
            );
        }

        Ok(())
    }
}
