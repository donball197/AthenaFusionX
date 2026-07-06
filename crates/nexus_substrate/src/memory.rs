use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, Row, SqlitePool};
use std::path::Path;
use nexus_search::VectorVault;
use tokio::sync::RwLock;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentMemory {
    pub id: i64,
    pub origin: String,
    pub directive: String,
    pub result: String,
    pub timestamp: String,
}

#[derive(FromRow, Debug, Clone)]
pub struct SutureRecoveryRecord {
    pub vmo_handle_id: i64,
    pub payload_blob: Vec<u8>,
    pub timestamp: i64,
}

pub struct MemoryPalace {
    pub pool: SqlitePool,
    pub vault: RwLock<VectorVault>,
}

impl MemoryPalace {
    pub async fn new(db_path: &str) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(db_path).parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        if !Path::new(db_path).exists() {
            std::fs::File::create(db_path).ok();
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}", db_path))
            .await?;

        // Stronger SQLite settings for stability
        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous=NORMAL;").execute(&pool).await?;
        sqlx::query("PRAGMA cache_size=-16000;").execute(&pool).await?;
        sqlx::query("PRAGMA foreign_keys=ON;").execute(&pool).await?;

        let palace = Self {
            pool,
            vault: RwLock::new(VectorVault::new(384, 3)),
        };

        palace.init_full_schema().await?;
        palace.hydrate_vault().await?;

        tracing::info!("MemoryPalace initialized with vector vault (dim=384)");
        Ok(palace)
    }

    pub async fn hydrate_vault(&self) -> Result<()> {
        let rows = sqlx::query("SELECT id, embedding FROM neural_context WHERE embedding IS NOT NULL")
            .fetch_all(&self.pool).await?;

        let mut v = self.vault.write().await;
        for row in rows {
            let id: i64 = row.get("id");
            let emb_str: Option<String> = row.get("embedding");
            if let Some(s) = emb_str {
                if let Ok(vec) = serde_json::from_str::<Vec<f32>>(&s) {
                    let _ = v.commit_state(id as u64, &vec);
                }
            }
        }
        Ok(())
    }

    pub async fn init_full_schema(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS neural_context (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                origin TEXT NOT NULL,
                directive TEXT NOT NULL,
                result TEXT NOT NULL,
                embedding TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            );"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(
                directive, result, content='neural_context', content_rowid='id'
            );"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_insights (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                summary TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            );"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS cognition_provenance (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vmo_handle_id INTEGER NOT NULL,
                payload_blob BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                thermal_at_suture REAL,
                reason TEXT NOT NULL
            );"
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plan_runs (
                id TEXT PRIMARY KEY,
                root_directive TEXT NOT NULL DEFAULT '',
                graph_json TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );"
        ).execute(&self.pool).await?;

        Ok(())
    }

    pub async fn store_memory(&self, origin: &str, directive: &str, result: &str, embedding: Option<Vec<f32>>) -> Result<()> {
        let emb_str = embedding.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());

        let mut tx = self.pool.begin().await?;
        let res = sqlx::query(
            "INSERT INTO neural_context (origin, directive, result, embedding) VALUES (?, ?, ?, ?)"
        )
        .bind(origin)
        .bind(directive)
        .bind(result)
        .bind(emb_str)
        .execute(&mut *tx).await?;

        let row_id = res.last_insert_rowid();

        sqlx::query("INSERT INTO memory_search (rowid, directive, result) VALUES (?, ?, ?)")
            .bind(row_id)
            .bind(directive)
            .bind(result)
            .execute(&mut *tx).await?;

        tx.commit().await?;

        if let Some(emb) = embedding {
            let mut v = self.vault.write().await;
            let _ = v.commit_state(row_id as u64, &emb);
        }
        Ok(())
    }

    // ... (rest of your methods: search_memories, semantic_search, purge_all, fetch_random_strands remain the same)
    pub async fn search_memories(&self, query: &str) -> Result<Vec<AgentMemory>> {
        let rows = sqlx::query(
            "SELECT n.id, n.origin, n.directive, n.result, n.timestamp
             FROM neural_context n
             JOIN memory_search m ON n.id = m.rowid
             WHERE memory_search MATCH ?
             ORDER BY rank LIMIT 10"
        ).bind(query).fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get(0),
            origin: row.get(1),
            directive: row.get(2),
            result: row.get(3),
            timestamp: row.get(4),
        }).collect())
    }

    pub async fn semantic_search(&self, query_vec: &[f32]) -> Result<Vec<AgentMemory>> {
        let ids = {
            let v = self.vault.read().await;
            v.recall_context(query_vec, 3).map(|(_, i)| i).unwrap_or_default()
        };

        if ids.is_empty() {
            return Ok(vec![]);
        }

        let id_binds = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!("SELECT id, origin, directive, result, timestamp FROM neural_context WHERE id IN ({})", id_binds);
        let mut q = sqlx::query(&query);
        for &id in &ids {
            q = q.bind(id as i64);
        }
        let rows = q.fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get("id"),
            origin: row.get("origin"),
            directive: row.get("directive"),
            result: row.get("result"),
            timestamp: row.get("timestamp"),
        }).collect())
    }

    pub async fn purge_all(&self) -> Result<()> {
        sqlx::query("DELETE FROM neural_context").execute(&self.pool).await?;
        sqlx::query("DELETE FROM memory_search").execute(&self.pool).await?;
        sqlx::query("DELETE FROM system_insights").execute(&self.pool).await?;

        {
            let mut v = self.vault.write().await;
            *v = VectorVault::new(384, 3);
        }
        Ok(())
    }

    pub async fn fetch_random_strands(&self, limit: i64) -> Result<Vec<AgentMemory>> {
        let rows = sqlx::query(
            "SELECT id, origin, directive, result, timestamp
             FROM neural_context
             ORDER BY RANDOM() LIMIT ?"
        ).bind(limit).fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get("id"),
            origin: row.get("origin"),
            directive: row.get("directive"),
            result: row.get("result"),
            timestamp: row.get("timestamp"),
        }).collect())
    }
}
