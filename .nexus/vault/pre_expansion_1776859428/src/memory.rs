/**
 * Project Nexus Singularity - v4.7.5 Memory Palace
 * Optimization: SQLite WAL mode for high-resonance concurrency.
 */
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

        // ⚡ EVOLUTION: Enable Write-Ahead Logging
        // This prevents the "Database is Locked" errors during multi-turn missions.
        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;

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

    pub async fn store_memory(
        &self,
        origin: &str,
        directive: &str,
        result: &str,
    ) -> Result<(), sqlx::Error> {
        let res =
            sqlx::query("INSERT INTO neural_context (origin, directive, result) VALUES (?, ?, ?)")
                .bind(origin)
                .bind(directive)
                .bind(result)
                .execute(&self.pool)
                .await?;

        let row_id = res.last_insert_rowid();
        sqlx::query("INSERT INTO memory_search (rowid, directive, result) VALUES (?, ?, ?)")
            .bind(row_id)
            .bind(directive)
            .bind(result)
            .execute(&self.pool)
            .await?;
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
                                FROM neural_context n
                                JOIN memory_search m ON n.id = m.rowid 
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
