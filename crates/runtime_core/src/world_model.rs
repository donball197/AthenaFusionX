use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

        // 1. Inject Repository File Tree Context (Safe Rust Iteration)
        let mut files = String::new();
        for entry in walkdir::WalkDir::new(".")
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let path_str = entry.path().to_string_lossy();
                if !path_str.contains("/.git/")
                    && !path_str.contains("/target/")
                    && !path_str.contains("/node_modules/")
                    && !path_str.contains("/.cold_storage/")
                {
                    files.push_str(&format!("{}\n", path_str));
                }
            }
        }
        // [FIX] Disabled truncated raw file tree to prevent 'apps/' vs 'crates/' alphabetical hallucination.
        // if !files.is_empty() {
        //     chunks.push("[REPOSITORY FILE TREE (Depth 4)]".to_string());
        //     chunks.push(files.chars().take(6000).collect());
        // }

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
