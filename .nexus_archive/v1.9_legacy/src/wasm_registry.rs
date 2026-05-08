// src/wasm_registry.rs — v1.0 Named WASM Cell Registry
// Stores compiled WASM cells by name so they can be reused across
// sessions without re-synthesis. Each entry tracks creation time,
// the original Rust source, and execution hit count.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmCell {
    pub name: String,
    pub wasm_b64: String,
    pub source_code: String,
    pub created_at: String,
    pub hit_count: i64,
}

pub struct WasmRegistry {
    pool: SqlitePool,
}

impl WasmRegistry {
    pub async fn new(db_path: &str) -> Result<Self> {
        if !std::path::Path::new(db_path).exists() {
            std::fs::File::create(db_path).ok();
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .connect(&format!("sqlite:{}", db_path))
            .await?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wasm_cells (
                name        TEXT PRIMARY KEY,
                wasm_b64    TEXT NOT NULL,
                source_code TEXT NOT NULL,
                created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
                hit_count   INTEGER DEFAULT 0
            )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// store: Upserts a named WASM cell.
    pub async fn store(&self, name: &str, wasm_b64: &str, source: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO wasm_cells (name, wasm_b64, source_code)
             VALUES (?, ?, ?)
             ON CONFLICT(name) DO UPDATE SET
               wasm_b64=excluded.wasm_b64,
               source_code=excluded.source_code,
               created_at=CURRENT_TIMESTAMP,
               hit_count=0",
        )
        .bind(name)
        .bind(wasm_b64)
        .bind(source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// get: Retrieves a cell by name and increments its hit count.
    pub async fn get(&self, name: &str) -> Result<Option<WasmCell>> {
        let row = sqlx::query(
            "SELECT name, wasm_b64, source_code, created_at, hit_count
             FROM wasm_cells WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            sqlx::query("UPDATE wasm_cells SET hit_count = hit_count + 1 WHERE name = ?")
                .bind(name)
                .execute(&self.pool)
                .await?;
            Ok(Some(WasmCell {
                name: r.get("name"),
                wasm_b64: r.get("wasm_b64"),
                source_code: r.get("source_code"),
                created_at: r.get("created_at"),
                hit_count: r.get("hit_count"),
            }))
        } else {
            Ok(None)
        }
    }

    /// list_all: Returns all stored cells (name + metadata, no wasm_b64).
    pub async fn list_all(&self) -> Result<Vec<WasmCell>> {
        let rows = sqlx::query(
            "SELECT name, '' as wasm_b64, source_code, created_at, hit_count
             FROM wasm_cells ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| WasmCell {
                name: r.get("name"),
                wasm_b64: String::new(),
                source_code: r.get("source_code"),
                created_at: r.get("created_at"),
                hit_count: r.get("hit_count"),
            })
            .collect())
    }

    /// delete: Removes a named cell.
    pub async fn delete(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM wasm_cells WHERE name = ?").bind(name).execute(&self.pool).await?;
        Ok(())
    }
}
