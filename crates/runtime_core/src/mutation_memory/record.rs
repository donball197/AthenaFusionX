use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationMemoryRecord {
    pub mutation_id: String,

    pub target_function: String,

    pub mutation_type: String,

    pub before_hash: String,

    pub after_hash: String,

    pub verification_status: String,

    pub tests_passed: bool,

    pub lesson: String,

    pub confidence: f32,
}

impl MutationMemoryRecord {
    pub fn successful_repair(function: &str, lesson: &str) -> Self {
        Self {
            mutation_id: uuid::Uuid::new_v4().to_string(),

            target_function: function.to_string(),

            mutation_type: "PatchFunction".into(),

            before_hash: "before".into(),

            after_hash: "after".into(),

            verification_status: "verified".into(),

            tests_passed: true,

            lesson: lesson.to_string(),

            confidence: 1.0,
        }
    }
}
