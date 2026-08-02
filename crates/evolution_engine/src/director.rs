use crate::genome::{
    CognitiveGenes, CriticGenes, CriticStyle, Genome, GenomeMetadata, PlanningGenes,
    PlanningStyle, ToolBias,
};
use crate::repository::GenomeRepository;
use chrono::Utc;
use uuid::Uuid;

#[derive(Default)]
pub struct EvolutionDirector {
    pub repository: GenomeRepository,
}

impl EvolutionDirector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bootstraps Generation 0 if no active genome exists.
    pub fn boot_genesis_genome(&mut self) -> &Genome {
        if self.repository.active.is_none() {
            let genesis_id = Uuid::new_v4();
            
            let genesis_genome = Genome {
                id: genesis_id,
                metadata: GenomeMetadata {
                    generation: 0,
                    parent_ids: vec![],
                    mutation_reason: "SYSTEM_INIT: Baseline Genesis Genome".to_string(),
                    created_at: Utc::now(),
                },
                genes: CognitiveGenes {
                    planning: PlanningGenes {
                        style: PlanningStyle::GraphOfThoughts,
                        decomposition_depth: 3,
                        branch_factor: 2,
                    },
                    critic: CriticGenes {
                        style: CriticStyle::Adversarial,
                        veto_threshold: 0.85,
                        evidence_weight: 1.0,
                    },
                    risk_threshold: 0.20,
                    tool_bias: ToolBias::Conservative,
                },
            };

            self.repository.save_genome(genesis_genome);
            self.repository.active = Some(genesis_id);
        }

        self.repository.get_active_genome().unwrap()
    }

    pub fn analyze_evidence(&self, _evidence: &str) -> Option<Genome> {
        // Future implementation: Evaluate fitness and trigger mutation
        None
    }
}
