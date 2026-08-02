use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildProposal {
    pub target: String,

    pub command: String,

    pub requires_approval: bool,

    pub reason: String,
}

impl BuildProposal {
    pub fn controlled(target: &str) -> Self {
        Self {
            target: target.to_string(),

            command: "cargo build --workspace".to_string(),

            requires_approval: true,

            reason: "Controlled workspace verification build".to_string(),
        }
    }
}
