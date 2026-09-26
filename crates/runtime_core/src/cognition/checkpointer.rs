use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

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
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    pub checkpoint_id: Uuid,
    pub parent_checkpoint_id: Option<Uuid>,
    pub plan_id: String,
    pub task_id: String,
    pub vmo_handles: Vec<u32>,
    pub channel_values: serde_json::Value,
    pub timestamp: i64,
}

#[derive(Clone)]
pub struct SqliteCheckpointer {
    pool: SqlitePool,
}

impl SqliteCheckpointer {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn ensure_checkpoints_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS state_checkpoints (
                checkpoint_id TEXT PRIMARY KEY,
                parent_checkpoint_id TEXT,
                plan_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                vmo_handles TEXT NOT NULL,
                channel_values TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_state_checkpoints_plan_id ON state_checkpoints(plan_id);
            CREATE INDEX IF NOT EXISTS idx_state_checkpoints_checkpoint_id ON state_checkpoints(checkpoint_id);
            "#,
        )
        .execute(&self.pool)
        .await?;

        ensure_column(
            &self.pool,
            "state_checkpoints",
            "taint_label",
            "TEXT NOT NULL DEFAULT 'I:2,S:1,E:2'",
        )
        .await?;

        Ok(())
    }

    pub async fn put(&self, checkpoint: StateCheckpoint) -> Result<()> {
        let vmo_json = serde_json::to_string(&checkpoint.vmo_handles)?;
        let channel_json = serde_json::to_string(&checkpoint.channel_values)?;
        let parent_id_str = checkpoint.parent_checkpoint_id.map(|id| id.to_string());

        sqlx::query(
            r#"
            INSERT INTO state_checkpoints (
                checkpoint_id, parent_checkpoint_id, plan_id, task_id, vmo_handles, channel_values, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(checkpoint_id) DO UPDATE SET
                parent_checkpoint_id = excluded.parent_checkpoint_id,
                plan_id = excluded.plan_id,
                task_id = excluded.task_id,
                vmo_handles = excluded.vmo_handles,
                channel_values = excluded.channel_values,
                timestamp = excluded.timestamp;
            "#,
        )
        .bind(checkpoint.checkpoint_id.to_string())
        .bind(parent_id_str)
        .bind(&checkpoint.plan_id)
        .bind(&checkpoint.task_id)
        .bind(vmo_json)
        .bind(channel_json)
        .bind(checkpoint.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, checkpoint_id: Uuid) -> Result<Option<StateCheckpoint>> {
        let row = sqlx::query(
            r#"
            SELECT checkpoint_id, parent_checkpoint_id, plan_id, task_id, vmo_handles, channel_values, timestamp
            FROM state_checkpoints
            WHERE checkpoint_id = ?
            "#,
        )
        .bind(checkpoint_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            let id_str: String = r.get("checkpoint_id");
            let parent_str: Option<String> = r.get("parent_checkpoint_id");
            let plan_id: String = r.get("plan_id");
            let task_id: String = r.get("task_id");
            let vmo_json: String = r.get("vmo_handles");
            let channel_json: String = r.get("channel_values");
            let timestamp: i64 = r.get("timestamp");

            let checkpoint_id = Uuid::parse_str(&id_str)?;
            let parent_checkpoint_id = match parent_str {
                Some(s) if !s.is_empty() => Some(Uuid::parse_str(&s)?),
                _ => None,
            };
            let vmo_handles: Vec<u32> = serde_json::from_str(&vmo_json)?;
            let channel_values: serde_json::Value = serde_json::from_str(&channel_json)?;

            Ok(Some(StateCheckpoint {
                checkpoint_id,
                parent_checkpoint_id,
                plan_id,
                task_id,
                vmo_handles,
                channel_values,
                timestamp,
            }))
        } else {
            Ok(None)
        }
    }
}
