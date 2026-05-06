// src/governance/protocol.rs — v2.6 [DETERMINISTIC_LOG]
use crate::agent_protocol::AgentAction;
use crate::fnv_hasher::FnvHasher as DefaultHasher;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MerkleLeaf {
    pub action: AgentAction,
    pub actor_id: String,
    pub timestamp: u64,
    pub hash: u64,
}

impl MerkleLeaf {
    pub fn new(action: AgentAction, actor_id: &str) -> Self {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut hasher = DefaultHasher::new();
        serde_json::to_vec(&action).unwrap_or_default().hash(&mut hasher);
        actor_id.hash(&mut hasher);
        ts.hash(&mut hasher);
        Self { action, actor_id: actor_id.to_string(), timestamp: ts, hash: hasher.finish() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GovernanceLog {
    pub history: Vec<MerkleLeaf>,
    pub root_hash: u64,
}

impl GovernanceLog {
    pub fn init() -> Self { Self::default() }

    pub fn record(&mut self, action: AgentAction, actor_id: &str) {
        let leaf = MerkleLeaf::new(action, actor_id);
        // Rolling Merkle root
        let mut hasher = DefaultHasher::new();
        self.root_hash.hash(&mut hasher);
        leaf.hash.hash(&mut hasher);
        self.root_hash = hasher.finish();
        self.history.push(leaf);
        
        if self.history.len() % 10 == 0 {
            info!("\x1b[34m[GOVERNANCE] Checkpoint: Root=0x{:x} (Size: {})\x1b[0m", self.root_hash, self.history.len());
        }
    }
}
