//! Probabilistic Memory & Causal Learning Engine for AthenaFusionX.
//! Calculates P(Relevance | Task) and maintains Positive Genomes vs Negative Constraints.

use crate::memory_cortex::MemoryCortex;
use crate::sovereign_core::SovereignOutcome;
use anyhow::Result;
use nexus_substrate::taint::TaintLabel;
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
            if let Ok(memories) = memory
                .semantic_search_scoped(
                    &query_vec,
                    3,
                    nexus_substrate::episode::RecallScope::ExplicitCrossEpisode,
                    &nexus_substrate::episode::EpisodeContext::default(),
                    nexus_substrate::taint::FlowPolicy::INTERNAL_SINK,
                )
                .await
            {
                for m in memories {
                    if has_marker(&m, "[HIGH_FITNESS_PATTERN]") || has_marker(&m, "[COMPILE_OK]") {
                        positive_priors.push(MemoryPrior {
                            directive: m.directive.clone(),
                            pattern_type: "POSITIVE_GENOME".to_string(),
                            evidence: m.result.lines().take(8).collect::<Vec<_>>().join("\n"),
                            confidence: 0.95,
                        });
                    } else if has_marker(&m, "[COMPILE_FAIL]")
                        || has_marker(&m, "[ACTION_GATE_BLOCKED]")
                        || has_marker(&m, "[MUTATION_REVERTED]")
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
                .store_memory(
                    "SovereignGenome",
                    directive,
                    &genome_payload,
                    TaintLabel::INTERNAL,
                    embedding,
                )
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
                    TaintLabel::INTERNAL,
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

/// A marker counts only when a result line STARTS with it, and never for rows
/// that came from file reads (source files merely mention these strings).
fn has_marker(m: &nexus_substrate::memory::AgentMemory, marker: &str) -> bool {
    let from_file_read = ["ReadFile", "ReadFunction", "ReadFileChunk"]
        .iter()
        .any(|o| m.origin.eq_ignore_ascii_case(o));
    !from_file_read && m.result.lines().any(|l| l.trim_start().starts_with(marker))
}

#[cfg(test)]
mod has_marker_tests {
    use super::has_marker;
    use nexus_substrate::memory::AgentMemory;

    fn mem(origin: &str, result: &str) -> AgentMemory {
        AgentMemory {
            id: 1,
            origin: origin.to_string(),
            directive: "d".to_string(),
            result: result.to_string(),
            taint_label: nexus_substrate::taint::TaintLabel::OBSERVED,
            episode: Default::default(),
            timestamp: "0".to_string(),
        }
    }

    #[test]
    fn file_read_rows_never_carry_markers() {
        assert!(!has_marker(&mem("ReadFile", "[COMPILE_OK] fine"), "[COMPILE_OK]"));
    }

    #[test]
    fn line_start_marker_from_a_real_source_counts() {
        assert!(has_marker(&mem("EvolutionEngine", "x\n[COMPILE_OK] fine"), "[COMPILE_OK]"));
    }

    #[test]
    fn marker_mentioned_mid_line_does_not_count() {
        assert!(!has_marker(&mem("ExecuteCommand", "println!(\"[COMPILE_OK]\");"), "[COMPILE_OK]"));
    }
}
