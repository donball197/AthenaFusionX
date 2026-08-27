use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

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
    /// Evidence produced by each completed DAG node this mission, keyed by node_id.
    /// Every node's semantic_query() call surfaces this automatically -- no agent
    /// has to think to ask for it, unlike search_memory which is opt-in per turn.
    pub node_evidence: Mutex<HashMap<String, String>>,
}

impl WorldModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Called once a DAG node finishes. Makes its evidence visible to every
    /// subsequent node's semantic_query() automatically, not just direct
    /// dependents via task_manager.rs's LINEAGE CONTEXT.
    pub fn record_node_evidence(&self, node_id: &str, objective: &str, evidence: &str) {
        if let Ok(mut map) = self.node_evidence.lock() {
            let entry = format!(
                "[NODE {} OBJECTIVE]: {}\n[NODE {} EVIDENCE]:\n{}",
                node_id, objective, node_id, evidence
            );
            map.insert(node_id.to_string(), entry);
        }
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

    pub async fn semantic_query(&self, mission: &str) -> Vec<String> {
        let mut chunks = Vec::new();

        // 1. Inject Repository File Tree Context (Safe Rust Iteration)
        let mut files = String::new();
        for entry in walkdir::WalkDir::new(".").max_depth(4).into_iter().filter_map(|e| e.ok()) {
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

        // Phase 1: surface every other completed node's evidence this mission,
        // ranked by simple keyword overlap with this node's own mission text.
        // Not embeddings yet (see search_memory / VectorVault for that gap) --
        // but real usage of `mission`, and it fixes agents working blind on
        // anything outside their direct DAG dependency chain.
        if let Ok(map) = self.node_evidence.lock() {
            if !map.is_empty() {
                let mission_words: Vec<String> = mission
                    .to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .map(|w| w.to_string())
                    .collect();

                let mut scored: Vec<(usize, &String)> = map
                    .values()
                    .map(|entry| {
                        let lower = entry.to_lowercase();
                        let score = mission_words.iter().filter(|w| lower.contains(w.as_str())).count();
                        (score, entry)
                    })
                    .collect();
                scored.sort_by_key(|b| std::cmp::Reverse(b.0));

                let mut evidence_block = String::from("[PRIOR MISSION EVIDENCE FROM OTHER NODES]\n");
                for (_, entry) in scored {
                    evidence_block.push_str(entry);
                    evidence_block.push_str("\n---\n");
                }
                chunks.push(evidence_block);
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
