//! Semantic Code Knowledge Graph (Code RAG)
//! Combines structural AST edges with 384-dim FastEmbed vector embeddings.
//! Enables semantic search ("find command injection") instead of just string search.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CodeNode {
    pub id: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub kind: String,
    pub source_snippet: String,
    pub embedding: String, // Stored as JSON array string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    pub node_id: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub kind: String,
    pub similarity: f32,
    pub source_snippet: String,
    // Note: verification_status is intentionally omitted here. 
    // The Cortical Loop will JOIN this result with OutcomeVerifier's state.
}

pub struct CodeGraph;

impl CodeGraph {
    /// Initializes the SQLite tables for the code graph.
    pub async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS code_nodes (
                id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                kind TEXT NOT NULL,
                source_snippet TEXT,
                embedding TEXT
            );
            CREATE TABLE IF NOT EXISTS code_edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL DEFAULT 1.0,
                PRIMARY KEY (source_id, target_id, edge_type)
            );"
        ).execute(pool).await?;
        Ok(())
    }

    /// Indexes the codebase by reading the symbol_graph.json, extracting source snippets,
    /// generating FastEmbeddings, and storing them in SQLite.
    pub async fn index_codebase(pool: &SqlitePool, graph_path: &Path, api_key: &str) -> Result<usize> {
        let content = fs::read_to_string(graph_path)?;
        let graph_data: serde_json::Value = serde_json::from_str(&content)?;
        
        let nodes = graph_data.get("nodes").and_then(|n| n.as_array()).cloned().unwrap_or_default();
        let mut indexed_count = 0;

        for node in nodes {
            let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let file_path = node.get("file_path").or_else(|| node.get("path")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let start_line = node.get("start_line").and_then(|v| v.as_u64()).unwrap_or(0) as i64;
            let end_line = node.get("end_line").and_then(|v| v.as_u64()).unwrap_or(0) as i64;
            let kind = node.get("kind").or_else(|| node.get("node_type")).and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();

            if file_path.is_empty() || id.is_empty() { continue; }

            // 1. Extract exact source snippet
            let file_content = fs::read_to_string(&file_path).unwrap_or_default();
            let lines: Vec<&str> = file_content.lines().collect();
            let start_idx = (start_line as usize).saturating_sub(1);
            let end_idx = (end_line as usize).min(lines.len());
            let snippet = if start_idx < end_idx {
                lines[start_idx..end_idx].join("\n")
            } else {
                String::new()
            };

            if snippet.is_empty() { continue; }

            // 2. Generate FastEmbed vector (uses local ONNX model, 0 network latency)
            let embedding = crate::vector_bridge::get_embedding(api_key, &snippet).await?;
            let emb_str = serde_json::to_string(&embedding)?;

            // 3. Upsert into SQLite
            sqlx::query(
                "INSERT INTO code_nodes (id, file_path, start_line, end_line, kind, source_snippet, embedding) 
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET 
                    file_path=excluded.file_path, start_line=excluded.start_line, 
                    end_line=excluded.end_line, kind=excluded.kind, 
                    source_snippet=excluded.source_snippet, embedding=excluded.embedding"
            )
            .bind(&id)
            .bind(&file_path)
            .bind(start_line)
            .bind(end_line)
            .bind(&kind)
            .bind(&snippet)
            .bind(&emb_str)
            .execute(pool).await?;

            indexed_count += 1;
        }

        Ok(indexed_count)
    }

    /// Queries the code graph using natural language. 
    /// Embeds the query, fetches all nodes (1184 is tiny), computes cosine similarity in-memory,
    /// and returns the top K matches.
    pub async fn semantic_search(pool: &SqlitePool, api_key: &str, query: &str, k: usize) -> Result<Vec<SemanticSearchResult>> {
        // 1. Embed the query
        let query_emb = crate::vector_bridge::get_embedding(api_key, query).await?;

        // 2. Fetch all nodes (with <2MB total RAM, this is perfectly safe and faster than SQL vector extensions)
        let rows = sqlx::query_as::<_, CodeNode>(
            "SELECT id, file_path, start_line, end_line, kind, source_snippet, embedding FROM code_nodes"
        ).fetch_all(pool).await?;

        // 3. Compute cosine similarity
        let mut results: Vec<SemanticSearchResult> = rows.into_iter().filter_map(|row| {
            let node_emb: Vec<f32> = serde_json::from_str(&row.embedding).unwrap_or_default();
            if node_emb.is_empty() { return None; }
            
            let sim = crate::vector_bridge::cosine_similarity(&query_emb, &node_emb);
            Some(SemanticSearchResult {
                node_id: row.id,
                file_path: row.file_path,
                start_line: row.start_line as usize,
                end_line: row.end_line as usize,
                kind: row.kind,
                similarity: sim,
                source_snippet: row.source_snippet,
            })
        }).collect();

        // 4. Sort by similarity descending and take top k
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);

        Ok(results)
    }
}
