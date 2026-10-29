use crate::genome::{FitnessVector, Genome};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GenomeEvaluation {
    pub benchmark_id: Uuid,
    pub fitness: FitnessVector,
    pub timestamp: DateTime<Utc>,
}

#[derive(Default)]
pub struct GenomeRepository {
    pub active: Option<Uuid>,
    pub lineage: HashMap<Uuid, Genome>,
    pub evaluations: HashMap<Uuid, Vec<GenomeEvaluation>>,
}

impl GenomeRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save_genome(&mut self, genome: Genome) {
        self.lineage.insert(genome.id, genome);
    }

    pub fn get_active_genome(&self) -> Option<&Genome> {
        self.active.and_then(|id| self.lineage.get(&id))
    }
}
