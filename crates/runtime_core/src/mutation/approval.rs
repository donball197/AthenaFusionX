use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MutationApproval {
    pub approved: bool,
    pub reason: String,
}
