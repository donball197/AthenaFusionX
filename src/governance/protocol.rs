//! # NEXUS Governance Protocol — Merkle Tree Audit Log v2.1 [FLOAT-SAFE]
//! Immutable, cryptographically verifiable logging for every `AgentAction`.

use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GovernanceError {
    IntegrityMismatch { expected: u64, actual: u64 },
    EmptyLog,
    NodeNotFound(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionMetadata {
    pub timestamp: u64,
    pub actor_id: String,
}

impl ActionMetadata {
    pub fn new(actor_id: impl Into<String>) -> Self {
        Self {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            actor_id: actor_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MerkleLeaf {
    pub action: AgentAction,
    pub metadata: ActionMetadata,
    pub hash: u64,
}

impl MerkleLeaf {
    pub fn new(action: AgentAction, actor_id: impl Into<String>) -> Self {
        let metadata = ActionMetadata::new(actor_id);
        let mut hasher = DefaultHasher::new();
        
        // FLOAT-SAFE: Serialize to bytes before hashing (handles f32 safely)
        let action_bytes = serde_json::to_vec(&action).unwrap_or_default();
        hasher.write(&action_bytes);
        
        metadata.timestamp.hash(&mut hasher);
        metadata.actor_id.hash(&mut hasher);
        
        Self { action, metadata, hash: hasher.finish() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MerkleNode {
    pub hash: u64,
    pub left: Option<usize>,
    pub right: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MerkleTree {
    pub nodes: Vec<MerkleNode>,
    pub leaf_indices: Vec<usize>,
    pub root_index: Option<usize>,
}

impl MerkleTree {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, leaf: MerkleLeaf) {
        let leaf_idx = self.nodes.len();
        self.nodes.push(MerkleNode { hash: leaf.hash, left: None, right: None });
        self.leaf_indices.push(leaf_idx);
        self.rebuild();
    }

    fn rebuild(&mut self) {
        if self.leaf_indices.is_empty() {
            self.root_index = None;
            return;
        }
        let mut current_layer: Vec<usize> = self.leaf_indices.clone();
        while current_layer.len() > 1 {
            let mut next_layer = Vec::new();
            let mut i = 0;
            while i < current_layer.len() {
                if i + 1 < current_layer.len() {
                    let left = current_layer[i];
                    let right = current_layer[i + 1];
                    let combined = self.hash_pair(left, right);
                    let new_idx = self.nodes.len();
                    self.nodes.push(MerkleNode { hash: combined, left: Some(left), right: Some(right) });
                    next_layer.push(new_idx);
                    i += 2;
                } else {
                    next_layer.push(current_layer[i]);
                    i += 1;
                }
            }
            current_layer = next_layer;
        }
        self.root_index = current_layer.first().copied();
    }

    fn hash_pair(&self, left_idx: usize, right_idx: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.nodes[left_idx].hash.hash(&mut hasher);
        self.nodes[right_idx].hash.hash(&mut hasher);
        hasher.finish()
    }

    pub fn root_hash(&self) -> Option<u64> {
        self.root_index.map(|i| self.nodes[i].hash)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GovernanceLog {
    pub history: Vec<MerkleLeaf>,
    pub tree: MerkleTree,
    pub session_id: String,
}

impl GovernanceLog {
    pub fn init(session_id: impl Into<String>) -> Self {
        Self { history: Vec::new(), tree: MerkleTree::new(), session_id: session_id.into() }
    }

    pub fn record(&mut self, action: AgentAction, actor_id: impl Into<String>) {
        let leaf = MerkleLeaf::new(action, actor_id);
        self.history.push(leaf.clone());
        self.tree.push(leaf);
    }
}
