use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceNode {
    pub path: String,
    pub kind: String,
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
        self.last_refresh =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
        Ok(())
    }

    pub fn get_summary(&self) -> String {
        format!("Workspace: {} nodes. Main binary: ide_backend.", self.nodes.len())
    }

    pub async fn get_topology() -> Option<String> {
        None
    }

    pub async fn semantic_query(&self, _mission: &str) -> Vec<String> {
        let mut chunks = Vec::new();

        // 1. Inject Repository File Tree Context
        if let Ok(out) = Command::new("find")
            .args([
                ".",
                "-maxdepth",
                "4",
                "-type",
                "f",
                "-not",
                "-path",
                "*/.git/*",
                "-not",
                "-path",
                "*/target/*",
                "-not",
                "-path",
                "*/node_modules/*",
                "-not",
                "-path",
                "*/.cold_storage/*",
            ])
            .output()
        {
            if out.status.success() {
                let files = String::from_utf8_lossy(&out.stdout);
                chunks.push("[REPOSITORY FILE TREE (Depth 4)]".to_string());
                chunks.push(files.chars().take(6000).collect());
            }
        }

        // 2. Inject Verified Machine-Readable Tool Contracts into Context
        chunks.push("\n[AVAILABLE WORKSPACE TOOL CONTRACTS]".to_string());
        for tool in crate::policy::TOOL_CONTRACTS {
            chunks.push(format!(
                "• Tool '{}': Usage -> {}\n  Binary -> {}",
                tool.name, tool.usage_pattern, tool.binary_path
            ));
        }

        chunks
    }
}
