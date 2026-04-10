/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


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
        sqlx::query("INSERT OR REPLACE INTO missions (id, goal, status, created_at, payload) VALUES (?, ?, ?, ?, ?)")
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
        let rows = sqlx::query("SELECT id, goal, status, created_at, payload FROM missions WHERE status = 'pending' ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(PersistentMission {
                id: row.get("id"),
                goal: row.get("goal"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                payload: row.get("payload"),
            });
        }
        Ok(results)
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
