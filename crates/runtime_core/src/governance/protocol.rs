// governance/protocol.rs — v2.0 [MERKLE_CHAIN]
use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn init() -> Self {
        Self::default()
    }

    /// record: Chains a new leaf into the Merkle root.
    /// Root = Hash(OldRoot ++ NewLeaf)
    pub fn record(&mut self, action: AgentAction, actor_id: &str) {
        let leaf = MerkleLeaf::new(action, actor_id);

        let mut hasher = DefaultHasher::new();
        self.root_hash.hash(&mut hasher);
        leaf.hash.hash(&mut hasher);
        self.root_hash = hasher.finish();

        self.history.push(leaf);

        if self.history.len().is_multiple_of(10) {
            info!(
                "\x1b[34m[GOVERNANCE] Checkpoint: Root=0x{:x} (Size: {})\x1b[0m",
                self.root_hash,
                self.history.len()
            );
        }
    }

    pub fn root(&self) -> u64 {
        self.root_hash
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}
