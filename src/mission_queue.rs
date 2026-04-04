use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio_rusqlite::Connection;
use std::sync::Arc;

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
    conn: Arc<Connection>,
}

impl MissionQueue {
    pub async fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path).await?;
        conn.call(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS missions (
                    id TEXT PRIMARY KEY,
                    goal TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    payload TEXT NOT NULL
                )",
                [],
            ).map_err(tokio_rusqlite::Error::from)?;
            Ok(())
        }).await?;
        Ok(Self { conn: Arc::new(conn) })
    }

    pub async fn enqueue(&self, mission: PersistentMission) -> Result<()> {
        let conn = self.conn.clone();
        conn.call(move |c| {
            c.execute(
                "INSERT OR REPLACE INTO missions (id, goal, status, created_at, payload) VALUES (?, ?, ?, ?, ?)",
                rusqlite::params![mission.id, mission.goal, mission.status, mission.created_at, mission.payload],
            ).map_err(tokio_rusqlite::Error::from)?;
            Ok(())
        }).await?;
        Ok(())
    }

    pub async fn get_pending(&self) -> Result<Vec<PersistentMission>> {
        let conn = self.conn.clone();
        let missions = conn.call(|c| {
            let mut stmt = c.prepare(
                "SELECT id, goal, status, created_at, payload FROM missions WHERE status = 'pending' ORDER BY created_at"
            ).map_err(tokio_rusqlite::Error::from)?;

            let rows = stmt.query_map([], |row| {
                Ok(PersistentMission {
                    id: row.get(0)?,
                    goal: row.get(1)?,
                    status: row.get(2)?,
                    created_at: row.get(3)?,
                    payload: row.get(4)?,
                })
            }).map_err(tokio_rusqlite::Error::from)?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r.map_err(tokio_rusqlite::Error::from)?);
            }
            Ok(results)
        }).await?;
        Ok(missions)
    }

    pub async fn update_status(&self, id: &str, status: &str) -> Result<()> {
        let conn = self.conn.clone();
        let id_str = id.to_string();
        let status_str = status.to_string();
        conn.call(move |c| {
            c.execute(
                "UPDATE missions SET status = ? WHERE id = ?",
                rusqlite::params![status_str, id_str],
            ).map_err(tokio_rusqlite::Error::from)?;
            Ok(())
        }).await?;
        Ok(())
    }
}
