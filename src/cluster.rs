use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ClusterNode {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub cpu: f32,
    pub ram: f32,
    pub last_seen: u64,
}

#[derive(Clone)]
pub struct ClusterMesh {
    nodes: Arc<RwLock<Vec<ClusterNode>>>,
}

impl ClusterMesh {
    pub fn new() -> Self {
        Self { nodes: Arc::new(RwLock::new(vec![])) }
    }

    pub fn register(&self, mut node: ClusterNode) {
        node.last_seen = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut guard = self.nodes.write();
        if let Some(existing) = guard.iter_mut().find(|n| n.id == node.id) {
            *existing = node;
        } else {
            guard.push(node);
        }
    }

    pub fn best_node(&self) -> Option<ClusterNode> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        self.nodes
            .read()
            .iter()
            .filter(|n| now - n.last_seen < 40)
            .cloned()
            .min_by(|a, b| (a.cpu + a.ram).partial_cmp(&(b.cpu + b.ram)).unwrap())
    }
}
