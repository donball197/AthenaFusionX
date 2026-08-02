use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub label: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub edge_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphMetadata {
    pub version: String,
    pub total_nodes: usize,
    pub total_edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: Vec<Edge>,
    pub metadata: GraphMetadata,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            metadata: GraphMetadata::default(),
        }
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
