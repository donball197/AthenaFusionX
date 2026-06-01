use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, Row, SqlitePool};
use std::path::Path;

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
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(
                directive, result, content='neural_context', content_rowid='id'
            );",
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_insights (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                summary TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        ).execute(&self.pool).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS cognition_provenance (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vmo_handle_id INTEGER NOT NULL,
                payload_blob BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                thermal_at_suture REAL,
                reason TEXT NOT NULL
            );",
        ).execute(&self.pool).await?;

        // ⚡ V3 EXECUTIVE CORTEX SCHEMA
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plan_runs (
                id TEXT PRIMARY KEY,
                root_directive TEXT NOT NULL,
                graph_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        ).execute(&self.pool).await?;

        // Dynamic schema evolution for legacy databases
        sqlx::query("ALTER TABLE plan_runs ADD COLUMN root_directive TEXT NOT NULL DEFAULT '';").execute(&self.pool).await.ok();
        sqlx::query("ALTER TABLE plan_runs ADD COLUMN graph_json TEXT NOT NULL DEFAULT '';").execute(&self.pool).await.ok();

        Ok(())
    }

    pub async fn store_memory(&self, origin: &str, directive: &str, result: &str, embedding: Option<Vec<f32>>) -> Result<(), sqlx::Error> {
        let emb_str = embedding.map(|v| serde_json::to_string(&v).unwrap_or_default());
        let mut tx = self.pool.begin().await?;
        let res = sqlx::query("INSERT INTO neural_context (origin, directive, result, embedding) VALUES (?, ?, ?, ?)")
            .bind(origin).bind(directive).bind(result).bind(emb_str).execute(&mut *tx).await?;
        let row_id = res.last_insert_rowid();
        sqlx::query("INSERT INTO memory_search (rowid, directive, result) VALUES (?, ?, ?)")
            .bind(row_id).bind(directive).bind(result).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn store_insight(&self, summary: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO system_insights (summary) VALUES (?)").bind(summary).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn fetch_all(&self) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp FROM neural_context ORDER BY id DESC LIMIT 50")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get("id"), origin: row.get("origin"), directive: row.get("directive"), result: row.get("result"), timestamp: row.get("timestamp"),
        }).collect())
    }

    pub async fn search_memories(&self, query: &str) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT n.id, n.origin, n.directive, n.result, n.timestamp FROM neural_context n JOIN memory_search m ON n.id = m.rowid WHERE memory_search MATCH ? ORDER BY rank LIMIT 10")
            .bind(query).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get(0), origin: row.get(1), directive: row.get(2), result: row.get(3), timestamp: row.get(4),
        }).collect())
    }

    pub async fn semantic_search(&self, query_vec: &[f32]) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, timestamp, embedding FROM neural_context WHERE embedding IS NOT NULL")
            .fetch_all(&self.pool).await?;
        let mut scored = Vec::new();
        for row in rows {
            let emb_str: Option<String> = row.get("embedding");
            if let Some(s) = emb_str {
                if let Ok(vec) = serde_json::from_str::<Vec<f32>>(&s) {
                    let score = crate::vector_bridge::cosine_similarity(query_vec, &vec);
                    scored.push((score, row));
                }
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(3);
        Ok(scored.into_iter().map(|(_, row)| AgentMemory {
            id: row.get("id"), origin: row.get("origin"), directive: row.get("directive"), result: row.get("result"), timestamp: row.get("timestamp"),
        }).collect())
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
        Ok(rows.into_iter().map(|row| AgentMemory {
            id: row.get("id"), origin: row.get("origin"), directive: row.get("directive"), result: row.get("result"), timestamp: row.get("timestamp"),
        }).collect())
    }

    // ── Provenance/suture methods ──
    pub async fn save_provenance(&self, handle: i64, payload: Vec<u8>, ts: u64) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO cognition_provenance (vmo_handle_id, payload_blob, timestamp, thermal_at_suture, reason) VALUES (?, ?, ?, 78.0, 'EMERGENCY_SUTURE')")
            .bind(handle).bind(payload).bind(ts as i64).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn restore_provenance(&self) -> Result<Option<SutureRecoveryRecord>, sqlx::Error> {
        sqlx::query_as::<_, SutureRecoveryRecord>("SELECT vmo_handle_id, payload_blob, timestamp FROM cognition_provenance WHERE reason = 'EMERGENCY_SUTURE' ORDER BY timestamp DESC LIMIT 1")
            .fetch_optional(&self.pool).await
    }

    pub async fn rotate_provenance_ledger(&self, max_retain: u32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM cognition_provenance WHERE reason = 'EMERGENCY_SUTURE' AND timestamp < (SELECT MIN(timestamp) FROM (SELECT timestamp FROM cognition_provenance WHERE reason = 'EMERGENCY_SUTURE' ORDER BY timestamp DESC LIMIT ?))")
            .bind(max_retain as i64).execute(&self.pool).await?;
        let rows = result.rows_affected();
        if rows > 0 { tracing::warn!("[MEMORY PALACE] Storage rotation complete. Recycled {} stale eMMC rows.", rows); }
        Ok(rows)
    }

    // ── V3 EXECUTIVE CORTEX DURABLE STATE ──
    pub async fn upsert_plan_run(&self, id: &str, root_directive: &str, graph_json: &str, status: &str, created_at: i64) -> Result<(), sqlx::Error> {
        let updated_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        sqlx::query(
            "INSERT INTO plan_runs (id, root_directive, graph_json, status, created_at, updated_at) 
             VALUES (?, ?, ?, ?, ?, ?) 
             ON CONFLICT(id) DO UPDATE SET 
                graph_json=excluded.graph_json, 
                status=excluded.status, 
                updated_at=excluded.updated_at"
        )
        .bind(id).bind(root_directive).bind(graph_json).bind(status).bind(created_at).bind(updated_at)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn recover_active_plans(&self) -> Result<Vec<(String, String, String)>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, root_directive, graph_json FROM plan_runs WHERE status IN ('Active', 'Suspended') ORDER BY updated_at ASC")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| (row.get("id"), row.get("root_directive"), row.get("graph_json"))).collect())
    }
}
