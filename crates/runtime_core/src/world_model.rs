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

pub struct MemorySideagent;

impl MemorySideagent {
    /// Distills raw evidence entries, stripping recon spam and capping token density
    pub fn distill(entry: &str) -> String {
        let mut clean_lines = Vec::new();
        for line in entry.lines() {
            let l = line.trim();
            if l.starts_with("[READ_CHUNK_OK]")
                || l.contains("No code matches found")
                || l.starts_with("FULL FILE CONTENT:")
                || l.contains("exit status: 0") && l.len() < 30
            {
                continue;
            }
            if clean_lines.len() < 15 {
                clean_lines.push(line);
            }
        }
        clean_lines.join("\n")
    }
}

#[derive(Default)]
pub struct WorldModel {
    pub nodes: HashMap<String, WorkspaceNode>,
    pub last_refresh: u64,
    pub node_evidence: Mutex<HashMap<String, String>>,
}

impl WorldModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_node_evidence(&self, node_id: &str, objective: &str, evidence: &str) {
        if let Ok(mut map) = self.node_evidence.lock() {
            let distilled = MemorySideagent::distill(evidence);
            let entry = format!(
                "[NODE {} OBJECTIVE]: {}\n[NODE {} DISTILLED FACTS]:\n{}",
                node_id, objective, node_id, distilled
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

        // Ranked & Distilled Node Evidence (Top 3 High-Density Entries)
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

                let mut evidence_block = String::from("[PRIOR MISSION EVIDENCE FROM OTHER NODES (Distilled)]\n");
                for (_, entry) in scored.into_iter().take(3) {
                    evidence_block.push_str(entry);
                    evidence_block.push_str("\n---\n");
                }
                chunks.push(evidence_block);
            }
        }

        // Available Tool Contracts
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
