use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentMemory {
    pub id: i64,
    pub origin: String,
    pub directive: String,
    pub result: String,
    pub timestamp: String,
}

pub struct MemoryPalace {
    pub pool: SqlitePool,
}

impl MemoryPalace {
    pub async fn new(db_path: &str) -> Result<Self, sqlx::Error> {
        if !Path::new(db_path).exists() {
            std::fs::File::create(db_path).ok();
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}", db_path))
            .await?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA cache_size=-16000;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous=NORMAL;").execute(&pool).await?;

        let palace = Self { pool };
        palace.init_full_schema().await?;
        Ok(palace)
    }

    pub async fn init_full_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS neural_context (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            origin TEXT NOT NULL,
            directive TEXT NOT NULL,
            result TEXT NOT NULL,
            embedding TEXT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(
            directive, result, content='neural_context', content_rowid='id'
        );",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_insights (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            summary TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// SUTURE: Atomic Transaction over FTS and Base Table
    pub async fn store_memory(
        &self,
        origin: &str,
        directive: &str,
        result: &str,
        embedding: Option<Vec<f32>>,
    ) -> Result<(), sqlx::Error> {
        let emb_str = embedding.map(|v| serde_json::to_string(&v).unwrap_or_default());

        let mut tx = self.pool.begin().await?;

        let res = sqlx::query(
            "INSERT INTO neural_context (origin, directive, result, embedding) VALUES (?, ?, ?, ?)",
        )
        .bind(origin)
        .bind(directive)
        .bind(result)
        .bind(emb_str)
        .execute(&mut *tx)
        .await?;

        let row_id = res.last_insert_rowid();
        sqlx::query("INSERT INTO memory_search (rowid, directive, result) VALUES (?, ?, ?)")
            .bind(row_id)
            .bind(directive)
            .bind(result)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn store_insight(&self, summary: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO system_insights (summary) VALUES (?)")
            .bind(summary)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn fetch_all(&self) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp FROM neural_context ORDER BY id DESC LIMIT 50")
            .fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| AgentMemory {
                id: row.get("id"),
                origin: row.get("origin"),
                directive: row.get("directive"),
                result: row.get("result"),
                timestamp: row.get("timestamp"),
            })
            .collect())
    }

    pub async fn search_memories(&self, query: &str) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT n.id, n.origin, n.directive, n.result, n.timestamp 
             FROM neural_context n JOIN memory_search m ON n.id = m.rowid 
             WHERE memory_search MATCH ? ORDER BY rank LIMIT 10",
        )
        .bind(query)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AgentMemory {
                id: row.get(0),
                origin: row.get(1),
                directive: row.get(2),
                result: row.get(3),
                timestamp: row.get(4),
            })
            .collect())
    }

    pub async fn semantic_search(
        &self,
        query_vec: &[f32],
    ) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp, embedding FROM neural_context WHERE embedding IS NOT NULL")
            .fetch_all(&self.pool).await?;

        let mut scored_mems = Vec::new();
        for row in rows {
            let emb_str: Option<String> = row.get("embedding");
            if let Some(s) = emb_str {
                if let Ok(vec) = serde_json::from_str::<Vec<f32>>(&s) {
                    let score = crate::vector_bridge::cosine_similarity(query_vec, &vec);
                    scored_mems.push((score, row));
                }
            }
        }

        scored_mems.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored_mems.truncate(3);

        Ok(scored_mems
            .into_iter()
            .map(|(_, row)| AgentMemory {
                id: row.get("id"),
                origin: row.get("origin"),
                directive: row.get("directive"),
                result: row.get("result"),
                timestamp: row.get("timestamp"),
            })
            .collect())
    }

    pub async fn purge_all(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM neural_context").execute(&self.pool).await?;
        sqlx::query("DELETE FROM memory_search").execute(&self.pool).await?;
        sqlx::query("DELETE FROM system_insights").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn fetch_random_strands(&self, limit: i64) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp FROM neural_context ORDER BY RANDOM() LIMIT ?")
            .bind(limit).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| AgentMemory {
                id: row.get("id"),
                origin: row.get("origin"),
                directive: row.get("directive"),
                result: row.get("result"),
                timestamp: row.get("timestamp"),
            })
            .collect())
    }
}
