use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PersistentMission {
    pub id: String,
    pub goal: String,
    pub status: String,
    pub created_at: i64,
    pub payload: String,
}

#[derive(Clone)]
pub struct MissionQueue {
    pool: SqlitePool,
}

impl MissionQueue {
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}", db_path))
            .await?;
        let queue = Self { pool };
        queue.init_schema().await?;
        Ok(queue)
    }

    async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS missions (
                id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                payload TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn enqueue(&self, mission: PersistentMission) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO missions (id, goal, status, created_at, payload) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&mission.id)
        .bind(&mission.goal)
        .bind(&mission.status)
        .bind(mission.created_at)
        .bind(&mission.payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_pending(&self) -> Result<Vec<PersistentMission>> {
        let rows = sqlx::query(
            "SELECT id, goal, status, created_at, payload FROM missions WHERE status = 'pending' ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PersistentMission {
                id: row.get("id"),
                goal: row.get("goal"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                payload: row.get("payload"),
            })
            .collect())
    }

    pub async fn update_status(&self, id: &str, status: &str) -> Result<()> {
        sqlx::query("UPDATE missions SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Commercial billing record (future monetization layer)
#[derive(Serialize, Deserialize, Clone)]
pub struct CommercialMission {
    pub client_id: String,
    pub mission_id: String,
    pub task: String,
    pub token_cost: u32,
    pub substrate_wear: f32,
    pub status: String,
}

impl CommercialMission {
    pub fn calculate_price(&self) -> f32 {
        (self.token_cost as f32 * 0.0002) + 0.10
    }
}
