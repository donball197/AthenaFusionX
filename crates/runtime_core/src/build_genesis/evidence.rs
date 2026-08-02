use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildEvidence {
    pub command: String,

    pub success: bool,

    pub lesson: String,
}
