use crate::episode::{EpisodeContext, RecallScope};
use crate::taint::TaintLabel;
/// Non-silent, idempotent column add. Checks PRAGMA table_info
/// first; runs ALTER only if the column is absent. Every error
/// is propagated: a security migration must not be swallowed
/// by `.ok()`.
async fn ensure_column(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), sqlx::Error> {
    let pragma = format!("PRAGMA table_info({})", table);
    let rows = sqlx::query(&pragma).fetch_all(pool).await?;
    let exists = rows.iter().any(|r| {
        let name: String = r.get("name");
        name == column
    });
    if exists {
        return Ok(());
    }
    let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition);
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

use nexus_search::VectorVault;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, Executor, FromRow, Row, SqlitePool};
use std::path::Path;
use std::sync::Arc;

/// Parse a stored taint label, accepting legacy string values.
pub fn parse_taint(s: &str) -> TaintLabel {
    TaintLabel::from_storage(s)
        .or_else(|| TaintLabel::from_legacy(s))
        .unwrap_or(TaintLabel::UNTRUSTED_PUBLIC)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentMemory {
    pub id: i64,
    pub origin: String,
    pub directive: String,
    pub result: String,
    pub taint_label: TaintLabel,
    pub episode: EpisodeContext,
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
    pub vault: Arc<Mutex<VectorVault>>,
}

impl MemoryPalace {
    pub async fn new(db_path: &str) -> Result<Self, sqlx::Error> {
        if !Path::new(db_path).exists() {
            std::fs::File::create(db_path).ok();
        }
        let connect_options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5))
            .pragma("cache_size", "-16000");

        let pool =
            SqlitePoolOptions::new().max_connections(5).connect_with(connect_options).await?;

        let vault = Arc::new(Mutex::new(VectorVault::new(384, 4)));

        let palace = Self { pool, vault };
        palace.init_full_schema().await?;
        palace.hydrate_vault().await?;

        Ok(palace)
    }

    async fn hydrate_vault(&self) -> Result<(), sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, embedding FROM neural_context WHERE embedding IS NOT NULL ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let count = rows.len();
        if count > 0 {
            let mut vault_guard = self.vault.lock();
            for row in rows {
                let id: i64 = row.get("id");
                let emb_str: Option<String> = row.get("embedding");
                if let Some(s) = emb_str {
                    match serde_json::from_str::<Vec<f32>>(&s) {
                        Ok(vec) if vec.len() == 384 => {
                            if let Err(e) = vault_guard.commit_state(id as u64, &vec) {
                                tracing::warn!(
                                    "[MEMORY PALACE] Failed to hydrate vector id {}: {}",
                                    id,
                                    e
                                );
                            }
                        }
                        Ok(vec) => {
                            tracing::warn!(
                                "[MEMORY PALACE] Skipping vector id {}: expected 384 dimensions, got {}",
                                id,
                                vec.len()
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[MEMORY PALACE] Skipping malformed embedding for id {}: {}",
                                id,
                                e
                            );
                        }
                    }
                }
            }
            tracing::info!(
                "[MEMORY PALACE] Scalable Vector Vault hydrated with {} strands.",
                count
            );
        }
        Ok(())
    }

    pub async fn init_full_schema(&self) -> Result<(), sqlx::Error> {
        self.pool
            .execute(
                "CREATE TABLE IF NOT EXISTS neural_context (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    origin TEXT NOT NULL,
                    directive TEXT NOT NULL,
                    result TEXT NOT NULL,
                    embedding TEXT,
                    taint_label TEXT NOT NULL DEFAULT 'I:0,S:0,E:0',
                    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
                );",
            )
            .await?;

        // ⚡ SAFE SCHEMA MIGRATION: Upgrades existing DBs dynamically
        self.pool
            .execute("ALTER TABLE neural_context ADD COLUMN taint_label TEXT NOT NULL DEFAULT 'I:0,S:0,E:0';")
            .await
            .ok();

        // Migrate legacy string labels to the packed encoding.
        self.pool
            .execute(
                "UPDATE neural_context\n                 SET taint_label = CASE taint_label\n                     WHEN 'Untrusted' THEN 'I:0,S:0,E:0'\n                     WHEN 'Observed'  THEN 'I:1,S:1,E:0'\n                     WHEN 'Mutated'   THEN 'I:1,S:1,E:1'\n                     WHEN 'Internal'  THEN 'I:2,S:1,E:2'\n                     ELSE taint_label\n                 END\n                 WHERE taint_label NOT LIKE 'I:%';",
            )
            .await
            .ok();

        // Episode provenance columns. Nullable: legacy rows have
        // NULL, meaning "episode unknown". No existing row is
        // modified or deleted by this migration.
        ensure_column(&self.pool, "neural_context", "plan_id", "TEXT NULL").await?;
        ensure_column(&self.pool, "neural_context", "task_id", "TEXT NULL").await?;
        ensure_column(&self.pool, "neural_context", "session_id", "TEXT NULL").await?;
        ensure_column(&self.pool, "neural_context", "mission_id", "TEXT NULL").await?;

        self.pool
            .execute(
                "CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(
                    directive, result, content='neural_context', content_rowid='id'
                );",
            )
            .await?;

        self.pool
            .execute(
                "CREATE TABLE IF NOT EXISTS system_insights (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    summary TEXT NOT NULL,
                    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
                );",
            )
            .await?;

        self.pool
            .execute(
                "CREATE TABLE IF NOT EXISTS cognition_provenance (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    vmo_handle_id INTEGER NOT NULL,
                    payload_blob BLOB NOT NULL,
                    timestamp INTEGER NOT NULL,
                    thermal_at_suture REAL,
                    reason TEXT NOT NULL
                );",
            )
            .await?;
        ensure_column(
            &self.pool,
            "cognition_provenance",
            "taint_label",
            "TEXT NOT NULL DEFAULT 'I:2,S:1,E:2'",
        )
        .await?;

        self.pool
            .execute(
                "CREATE TABLE IF NOT EXISTS plan_runs (
                    id TEXT PRIMARY KEY,
                    root_directive TEXT NOT NULL,
                    graph_json TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );",
            )
            .await?;

        self.pool
            .execute("ALTER TABLE plan_runs ADD COLUMN root_directive TEXT NOT NULL DEFAULT '';")
            .await
            .ok();
        self.pool
            .execute("ALTER TABLE plan_runs ADD COLUMN graph_json TEXT NOT NULL DEFAULT '';")
            .await
            .ok();

        ensure_column(
            &self.pool,
            "plan_runs",
            "taint_label",
            "TEXT NOT NULL DEFAULT 'I:2,S:1,E:2'",
        )
        .await?;

        Ok(())
    }

    /// Backwards-compatible entry point. Episode defaults to
    /// all-NULL ("episode unknown"). Prefer store_memory_with_episode
    /// at call sites that know their plan/task/session/mission.
    pub async fn store_memory(
        &self,
        origin: &str,
        directive: &str,
        result: &str,
        taint_label: TaintLabel,
        embedding: Option<Vec<f32>>,
    ) -> Result<(), sqlx::Error> {
        self.store_memory_with_episode(
            origin,
            directive,
            result,
            taint_label,
            embedding,
            &EpisodeContext::default(),
        )
        .await
    }

    pub async fn store_memory_with_episode(
        &self,
        origin: &str,
        directive: &str,
        result: &str,
        taint_label: TaintLabel,
        embedding: Option<Vec<f32>>,
        episode: &EpisodeContext,
    ) -> Result<(), sqlx::Error> {
        // Deduplication: Check if most recent entry for (origin, directive) has identical content
        let existing = sqlx::query(
            "SELECT id, result, taint_label FROM neural_context WHERE origin = ? AND directive = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(origin)
        .bind(directive)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = existing {
            let prev_result: String = row.get("result");
            if prev_result == result {
                let existing_id: i64 = row.get("id");
                let prev_raw: String = row.get("taint_label");
                let prev = parse_taint(&prev_raw);
                let merged = prev.join(taint_label);
                if merged != prev {
                    sqlx::query("UPDATE neural_context SET taint_label = ? WHERE id = ?")
                        .bind(merged.to_storage())
                        .bind(existing_id)
                        .execute(&self.pool)
                        .await?;
                }
                sqlx::query("UPDATE neural_context SET timestamp = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(existing_id)
                    .execute(&self.pool)
                    .await?;
                return Ok(());
            }
        }

        if let Some(ref vec) = embedding {
            if vec.len() != 384 {
                return Err(sqlx::Error::Protocol(format!(
                    "Embedding dimension mismatch: expected 384, got {}",
                    vec.len()
                )));
            }
        }

        let emb_str = embedding.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());
        let mut tx = self.pool.begin().await?;

        let res = sqlx::query(
            "INSERT INTO neural_context (origin, directive, result, embedding, taint_label, plan_id, task_id, session_id, mission_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(origin)
        .bind(directive)
        .bind(result)
        .bind(emb_str)
        .bind(taint_label.to_storage())
        .bind(episode.plan_id.as_deref())
        .bind(episode.task_id.as_deref())
        .bind(episode.session_id.as_deref())
        .bind(episode.mission_id.as_deref())
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

        if let Some(vec) = embedding {
            let mut vault = self.vault.lock();
            if let Err(e) = vault.commit_state(row_id as u64, &vec) {
                tracing::error!(
                    "[MEMORY PALACE] Vector index commit failed for memory id {}: {}",
                    row_id,
                    e
                );
            }
        }

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
        let rows = sqlx::query("SELECT id, origin, directive, result, taint_label, timestamp, plan_id, task_id, session_id, mission_id FROM neural_context ORDER BY id DESC LIMIT 50")
            .fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| AgentMemory {
                id: row.get("id"),
                origin: row.get("origin"),
                directive: row.get("directive"),
                result: row.get("result"),
                taint_label: parse_taint(&row.get::<String, _>("taint_label")),
                episode: EpisodeContext {
                    plan_id: row.get("plan_id"),
                    task_id: row.get("task_id"),
                    session_id: row.get("session_id"),
                    mission_id: row.get("mission_id"),
                },
                timestamp: row.get("timestamp"),
            })
            .collect())
    }

    pub async fn search_memories(&self, query: &str) -> Result<Vec<AgentMemory>, sqlx::Error> {
        // FTS5 MATCH interprets tokens containing ':' as query syntax,
        // including column selectors such as "OBJECTIVE:".
        //
        // Missions are natural language, so quote each token before passing
        // it to MATCH. OR preserves broad lexical retrieval while preventing
        // mission text from becoming FTS5 control syntax.
        let fts_query = query
            .split_whitespace()
            .filter_map(|token| {
                let token = token.trim_matches(|c: char| {
                    !c.is_alphanumeric() && !matches!(c, '_' | '-' | '.' | '/')
                });

                if token.is_empty() {
                    return None;
                }

                let escaped = token.replace('"', "\"\"");
                Some(format!("\"{}\"", escaped))
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        if fts_query.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT n.id, n.origin, n.directive, n.result, n.taint_label, n.timestamp, n.plan_id, n.task_id, n.session_id, n.mission_id              FROM neural_context n              JOIN memory_search m ON n.id = m.rowid              WHERE memory_search MATCH ?              ORDER BY rank              LIMIT 10",
        )
        .bind(&fts_query)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AgentMemory {
                id: row.get("id"),
                origin: row.get("origin"),
                directive: row.get("directive"),
                result: row.get("result"),
                taint_label: parse_taint(&row.get::<String, _>("taint_label")),
                episode: EpisodeContext {
                    plan_id: row.get("plan_id"),
                    task_id: row.get("task_id"),
                    session_id: row.get("session_id"),
                    mission_id: row.get("mission_id"),
                },
                timestamp: row.get("timestamp"),
            })
            .collect())
    }

    /// Taint- and episode-scoped recall.
    ///
    /// Scope gates episode eligibility; policy gates taint.
    /// Retrieval slack of 4x accounts for entries filtered out
    /// by either gate.
    pub async fn semantic_search_scoped(
        &self,
        query_vec: &[f32],
        limit: usize,
        scope: RecallScope,
        current: &EpisodeContext,
        policy: crate::taint::FlowPolicy,
    ) -> anyhow::Result<Vec<AgentMemory>> {
        let candidates = self.semantic_search(query_vec, limit.saturating_mul(4)).await?;
        let mut admitted = Vec::with_capacity(limit);
        for m in candidates {
            if m.taint_label.flow_to(policy).is_err() {
                continue;
            }
            let eligible = match scope {
                RecallScope::ExplicitCrossEpisode => true,
                RecallScope::CurrentEpisode => current.same_session(&m.episode),
                RecallScope::CurrentMission => current.same_mission(&m.episode),
            };
            if !eligible {
                continue;
            }
            admitted.push(m);
            if admitted.len() >= limit {
                break;
            }
        }
        Ok(admitted)
    }

    pub async fn semantic_search(
        &self,
        query_vec: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<AgentMemory>> {
        let matched_ids = {
            let vault = self.vault.lock();
            vault
                .recall_context(query_vec, limit)
                .map(|(_scores, ids)| ids)
                .map_err(|e| anyhow::anyhow!("VECTOR_RECALL_FAILED: {}", e))?
        };

        if matched_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut memories = Vec::new();
        for id in matched_ids {
            let row = sqlx::query(
                "SELECT id, origin, directive, result, taint_label, timestamp, plan_id, task_id, session_id, mission_id FROM neural_context WHERE id = ?",
            )
            .bind(id as i64)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(r) = row {
                memories.push(AgentMemory {
                    id: r.get("id"),
                    origin: r.get("origin"),
                    directive: r.get("directive"),
                    result: r.get("result"),
                    taint_label: parse_taint(&r.get::<String, _>("taint_label")),
                    episode: EpisodeContext {
                        plan_id: r.get("plan_id"),
                        task_id: r.get("task_id"),
                        session_id: r.get("session_id"),
                        mission_id: r.get("mission_id"),
                    },
                    timestamp: r.get("timestamp"),
                });
            }
        }
        Ok(memories)
    }

    pub async fn purge_all(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM neural_context").execute(&self.pool).await?;
        sqlx::query("DELETE FROM memory_search").execute(&self.pool).await?;
        sqlx::query("DELETE FROM system_insights").execute(&self.pool).await?;

        let mut vault = self.vault.lock();
        *vault = VectorVault::new(384, 4);

        Ok(())
    }

    pub async fn fetch_random_strands(&self, limit: i64) -> Result<Vec<AgentMemory>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, origin, directive, result, taint_label, timestamp, plan_id, task_id, session_id, mission_id FROM neural_context ORDER BY RANDOM() LIMIT ?")
            .bind(limit).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| AgentMemory {
                id: row.get("id"),
                origin: row.get("origin"),
                directive: row.get("directive"),
                result: row.get("result"),
                taint_label: parse_taint(&row.get::<String, _>("taint_label")),
                episode: EpisodeContext {
                    plan_id: row.get("plan_id"),
                    task_id: row.get("task_id"),
                    session_id: row.get("session_id"),
                    mission_id: row.get("mission_id"),
                },
                timestamp: row.get("timestamp"),
            })
            .collect())
    }

    pub async fn save_provenance(
        &self,
        handle: i64,
        payload: Vec<u8>,
        ts: u64,
    ) -> Result<(), sqlx::Error> {
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
        if rows > 0 {
            tracing::warn!(
                "[MEMORY PALACE] Storage rotation complete. Recycled {} stale eMMC rows.",
                rows
            );
        }
        Ok(rows)
    }

    pub async fn upsert_plan_run(
        &self,
        id: &str,
        root_directive: &str,
        graph_json: &str,
        status: &str,
        created_at: i64,
    ) -> Result<(), sqlx::Error> {
        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        sqlx::query(
            "INSERT INTO plan_runs (id, root_directive, graph_json, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                graph_json=excluded.graph_json,
                status=excluded.status,
                updated_at=excluded.updated_at",
        )
        .bind(id)
        .bind(root_directive)
        .bind(graph_json)
        .bind(status)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn recover_active_plans(&self) -> Result<Vec<(String, String, String)>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, root_directive, graph_json FROM plan_runs WHERE status IN ('Active', 'Suspended') ORDER BY updated_at ASC")
            .fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get("id"), row.get("root_directive"), row.get("graph_json")))
            .collect())
    }
}
