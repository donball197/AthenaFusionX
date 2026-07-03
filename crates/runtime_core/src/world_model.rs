use std::collections::HashMap;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceNode {
    pub path: String,
    pub kind: String, // "crate", "bin", "module", "file"
    pub deps: Vec<String>,
}

#[derive(Default)]
pub struct WorldModel {
    pub nodes: HashMap<String, WorkspaceNode>,
    pub last_refresh: u64,
}

impl WorldModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn refresh(&mut self) -> Result<()> {
        // Minimal implementation — expand with `cargo metadata` parsing later
        self.nodes.insert("ide_backend".to_string(), WorkspaceNode {
            path: "bins/ide_backend".to_string(),
            kind: "bin".to_string(),
            deps: vec!["runtime_core".to_string(), "nexus_substrate".to_string()],
        });
        self.last_refresh = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        Ok(())
    }

    pub fn get_summary(&self) -> String {
        format!("Workspace: {} nodes. Main binary: ide_backend. Key deps: runtime_core, nexus_substrate.", 
                self.nodes.len())
    }
}

impl WorldModel {
    pub async fn get_topology() -> Option<String> {
        None
    }

    pub async fn semantic_query(&self, _mission: &str) -> Vec<String> {
        Vec::new()
    }
}
