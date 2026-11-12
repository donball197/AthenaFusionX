// governance/protocol.rs — v3.0 [SECURE_MERKLE_CHAIN]
use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleLeaf {
    action: AgentAction,
    actor_id: String,
    timestamp: u64,
    hash: String,
}

impl MerkleLeaf {
    pub fn new(action: AgentAction, actor_id: &str) -> Self {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        // Prevent serialization failures from silently breaking the chain
        let action_bytes = serde_json::to_vec(&action)
            .unwrap_or_else(|_| b"CRITICAL_SERIALIZATION_FAILURE".to_vec());

        let mut hasher = Sha256::new();
        hasher.update(&action_bytes);
        hasher.update(actor_id.as_bytes());
        hasher.update(ts.to_be_bytes());

        Self {
            action,
            actor_id: actor_id.to_string(),
            timestamp: ts,
            hash: format!("{:x}", hasher.finalize()),
        }
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceLog {
    history: VecDeque<MerkleLeaf>,
    root_hash: String,
    max_capacity: usize,
}

impl Default for GovernanceLog {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(1000),
            root_hash: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            max_capacity: 1000,
        }
    }
}

impl GovernanceLog {
    pub fn init() -> Self {
        Self::default()
    }

    pub fn init_shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::init()))
    }

    /// record: Chains a new leaf into the Merkle root.
    /// Root = SHA256(OldRoot ++ NewLeaf)
    pub fn record(&mut self, action: AgentAction, actor_id: &str) {
        let leaf = MerkleLeaf::new(action, actor_id);

        let mut hasher = Sha256::new();
        hasher.update(self.root_hash.as_bytes());
        hasher.update(leaf.hash.as_bytes());
        self.root_hash = format!("{:x}", hasher.finalize());

        if self.history.len() >= self.max_capacity {
            self.history.pop_front();
        }

        self.history.push_back(leaf);

        if self.history.len() % 10 == 0 {
            info!(
                "[GOVERNANCE] Checkpoint: Root={} (Size: {})",
                self.root_hash,
                self.history.len()
            );
        }
    }

    pub fn root(&self) -> &str {
        &self.root_hash
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

pub static GOVERNANCE: LazyLock<Mutex<GovernanceLog>> =
    LazyLock::new(|| Mutex::new(GovernanceLog::init()));
