/**
 * Project Nexus Singularity - v4.5.1 Memory Palace (SQLx + FTS5)
 */
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
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

        let palace = Self { pool };
        palace.init_full_schema().await?;
        Ok(palace)
    }

    pub async fn init_full_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query("CREATE TABLE IF NOT EXISTS neural_context (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            origin TEXT NOT NULL,
            directive TEXT NOT NULL,
            result TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        );").execute(&self.pool).await?;

        sqlx::query("CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(
            directive, result, content='neural_context', content_rowid='id'
        );").execute(&self.pool).await?;
        
        sqlx::query("CREATE TABLE IF NOT EXISTS healing_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            layer TEXT NOT NULL,
            success INTEGER NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            cvss REAL,
            notes TEXT
        );").execute(&self.pool).await?;

        Ok(())
    }

    // ⚡ THE MISSING BINDING RESTORED
    pub async fn store_memory(&self, origin: &str, directive: &str, result: &str) -> Result<(), sqlx::Error> {
        let res = sqlx::query("INSERT INTO neural_context (origin, directive, result) VALUES (?, ?, ?)")
            .bind(origin).bind(directive).bind(result).execute(&self.pool).await?;
        let row_id = res.last_insert_rowid();
        sqlx::query("INSERT INTO memory_search (rowid, directive, result) VALUES (?, ?, ?)")
            .bind(row_id).bind(directive).bind(result).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn fetch_all(&self) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp FROM neural_context ORDER BY id DESC LIMIT 50")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get("id"), origin: row.get("origin"), directive: row.get("directive"),
            result: row.get("result"), timestamp: row.get("timestamp"),
        }).collect())
    }

    pub async fn search_memories(&self, query: &str) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT neural_context.id, neural_context.origin, neural_context.directive, neural_context.result, neural_context.timestamp
                                FROM neural_context
                                JOIN memory_search ON neural_context.id = memory_search.rowid
                                WHERE memory_search MATCH ? ORDER BY rank LIMIT 5")
            .bind(query).fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(|r| AgentMemory {
            id: r.get("id"), origin: r.get("origin"), directive: r.get("directive"),
            result: r.get("result"), timestamp: r.get("timestamp")
        }).collect())
    }

    pub async fn purge_all(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM neural_context").execute(&self.pool).await?;
        sqlx::query("DELETE FROM memory_search").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn fetch_random_strands(&self, limit: i64) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp FROM neural_context ORDER BY RANDOM() LIMIT ?")
            .bind(limit).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get("id"), origin: row.get("origin"), directive: row.get("directive"),
            result: row.get("result"), timestamp: row.get("timestamp"),
        }).collect())
    }

    pub async fn fetch_oldest_strands(&self, limit: i64) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp FROM neural_context ORDER BY id ASC LIMIT ?")
            .bind(limit).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get("id"), origin: row.get("origin"), directive: row.get("directive"),
            result: row.get("result"), timestamp: row.get("timestamp"),
        }).collect())
    }

    pub async fn update_strand_summary(&self, id: i64, summary: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE neural_context SET result = ? WHERE id = ?")
            .bind(summary).bind(id).execute(&self.pool).await?;
        sqlx::query("INSERT INTO memory_search(memory_search, rowid, directive, result) VALUES('delete', ?, ?, ?)")
            .bind(id).execute(&self.pool).await.ok(); 
        sqlx::query("INSERT INTO memory_search(rowid, directive, result) SELECT id, directive, result FROM neural_context WHERE id = ?")
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_success_rate_for_layer(&self, layer: &str) -> f32 {
        let result = sqlx::query("SELECT CAST(SUM(success) AS REAL) * 1.0 / COUNT(*) as rate FROM healing_metrics WHERE layer = ?")
            .bind(layer).fetch_optional(&self.pool).await;
        match result {
            Ok(Some(row)) => row.get::<Option<f32>, _>("rate").unwrap_or(0.78),
            _ => 0.78,
        }
    }
}
