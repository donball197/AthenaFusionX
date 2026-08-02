//! Mutation Evidence
//!
//! Captures objective evidence surrounding a code mutation.
//! Used by Mutation Critic for evaluation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationEvidence {
    pub target_file: String,

    pub target_function: String,

    pub before_hash: String,

    pub after_hash: String,

    pub cargo_check_passed: bool,

    pub tests_passed: bool,

    pub ast_similarity: f32,

    pub changed_regions: usize,

    pub timestamp: i64,
}

impl MutationEvidence {
    pub fn verified(&self) -> bool {
        self.cargo_check_passed
            && self.tests_passed
            && self.ast_similarity >= 0.85
            && self.changed_regions <= 1
    }
}
