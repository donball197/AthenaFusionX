use petgraph::graph::{DiGraph, NodeIndex};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstNode {
    pub name: String,
    pub node_type: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstFile {
    pub file_path: String,
    pub nodes: Vec<AstNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolNode {
    pub id: String,
    pub kind: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolEdge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub confidence: f32,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableGraph {
    pub nodes: Vec<SymbolNode>,
    pub edges: Vec<SymbolEdge>,
}

pub fn compute_edges(files: Vec<AstFile>) -> SerializableGraph {
    let mut graph = DiGraph::<SymbolNode, String>::new();
    let mut node_indices: HashMap<String, NodeIndex> = HashMap::new();

    // Global dictionary mapping symbol name -> unique node ID
    let mut symbol_dictionary: HashMap<String, String> = HashMap::new();

    // Pass 1: Hydrate nodes
    for file in &files {
        for ast_node in &file.nodes {
            let unique_id = format!("{}::{}", file.file_path, ast_node.name);

            let node = SymbolNode {
                id: unique_id.clone(),
                kind: ast_node.node_type.clone(),
                file_path: file.file_path.clone(),
            };

            let idx = graph.add_node(node);
            node_indices.insert(unique_id.clone(), idx);

            // Track name to ID for fast scanning lookup
            symbol_dictionary.insert(ast_node.name.clone(), unique_id);
        }
    }

    // Pass 2: Parallel Fast Edge Discovery (Phase 5A)
    let edges = Mutex::new(Vec::new());

    files.par_iter().for_each(|file| {
        if let Ok(content) = fs::read_to_string(&file.file_path) {
            let lines: Vec<&str> = content.lines().collect();

            for (line_idx, line) in lines.iter().enumerate() {
                let current_line_num = line_idx + 1;

                // Identify which AstNode owns this line
                if let Some(owning_node) = file
                    .nodes
                    .iter()
                    .find(|n| current_line_num >= n.start_line && current_line_num <= n.end_line)
                {
                    let caller_id = format!("{}::{}", file.file_path, owning_node.name);

                    // Fast tokenization by splitting on non-alphanumeric chars
                    let tokens: Vec<&str> =
                        line.split(|c: char| !c.is_alphanumeric() && c != '_').collect();

                    for token in tokens {
                        // Skip self-references and empty tokens
                        if token.is_empty() || token == owning_node.name {
                            continue;
                        }

                        // If the token matches a known symbol, create a probabilistic edge
                        if let Some(callee_id) = symbol_dictionary.get(token) {
                            let new_edge = SymbolEdge {
                                from: caller_id.clone(),
                                to: callee_id.clone(),
                                edge_type: "uses".to_string(),
                                confidence: 0.41,
                                evidence: "Regex_Line_Scan".to_string(),
                            };

                            edges.lock().unwrap().push(new_edge);
                        }
                    }
                }
            }
        }
    });

    // Deduplicate edges and format output
    let mut final_edges = edges.into_inner().unwrap();
    final_edges.sort_by(|a, b| a.from.cmp(&b.from).then_with(|| a.to.cmp(&b.to)));
    final_edges.dedup_by(|a, b| a.from == b.from && a.to == b.to);

    let mut serializable_nodes = Vec::new();
    for node_weight in graph.node_weights() {
        serializable_nodes.push(node_weight.clone());
    }

    SerializableGraph { nodes: serializable_nodes, edges: final_edges }
}
