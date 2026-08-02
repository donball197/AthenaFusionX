use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildNode {
    pub file: String,
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct BuildGraph {
    pub nodes: Vec<BuildNode>,
}

impl Default for BuildGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildGraph {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add(&mut self, file: &str, action: &str) {
        self.nodes.push(BuildNode { file: file.into(), action: action.into() });
    }
}
