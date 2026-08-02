use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildEvidence {
    pub artifact: String,

    pub verified: bool,

    pub lesson: String,
}
