import difflib

TARGET = "crates/runtime_core/src/governance/protocol.rs"
EXPECTED = '''use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
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
    pub fn init_shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::init()))
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
                "[GOVERNANCE] Checkpoint: Root=0x{:x} (Size: {})",
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
}'''

with open(TARGET, "r", encoding="utf-8") as f:
    actual_full = f.read()

# find where the expected block would start in the actual file
idx = actual_full.find("use crate::agent_protocol::AgentAction;")
if idx == -1:
    print("Could not even find the starting line 'use crate::agent_protocol::AgentAction;' in the file at all.")
else:
    actual_slice = actual_full[idx:idx+len(EXPECTED)+200]
    diff = difflib.unified_diff(
        EXPECTED.splitlines(keepends=True),
        actual_slice.splitlines(keepends=True),
        fromfile="expected_anchor",
        tofile="actual_file",
        lineterm=""
    )
    diff_lines = list(diff)
    if not diff_lines:
        print("NO DIFFERENCE FOUND -- anchor should match. (weird, re-run the real script)")
    else:
        print("DIFFERENCES FOUND:")
        for line in diff_lines[:60]:
            print(line)
