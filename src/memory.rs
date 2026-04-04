use serde::{Serialize, Deserialize};
use tokio_rusqlite::Connection;
use std::sync::Arc;
use anyhow::Result;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub category: String, // e.g., "Business Goal", "Architectural Rule", "DNA"
    pub timestamp: i64,
}

pub struct MemoryPalace {
    conn: Arc<Connection>,
}

impl MemoryPalace {
    pub async fn init(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path).await?;
        conn.call(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS memories (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    category TEXT NOT NULL,
                    timestamp INTEGER NOT NULL
                )",
                [],
            ).map_err(tokio_rusqlite::Error::from)?;
            Ok(())
        }).await?;
        
        Ok(Self { conn: Arc::new(conn) })
    }

    pub async fn store(&self, entry: MemoryEntry) -> Result<()> {
        let conn = self.conn.clone();
        conn.call(move |c| {
            c.execute(
                "INSERT OR REPLACE INTO memories (key, value, category, timestamp) VALUES (?, ?, ?, ?)",
                rusqlite::params![entry.key, entry.value, entry.category, entry.timestamp],
            ).map_err(tokio_rusqlite::Error::from)?;
            Ok(())
        }).await?;
        Ok(())
    }

    pub async fn fetch_all(&self) -> Result<Vec<MemoryEntry>> {
        let conn = self.conn.clone();
        conn.call(|c| {
            let mut stmt = c.prepare("SELECT key, value, category, timestamp FROM memories ORDER BY category ASC")?;
            let rows = stmt.query_map([], |row| {
                Ok(MemoryEntry {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    category: row.get(2)?,
                    timestamp: row.get(3)?,
                })
            })?;
            let mut results = Vec::new();
            for r in rows { results.push(r?); }
            Ok(results)
        }).await.map_err(anyhow::Error::from)
    }

    pub async fn get_system_context(&self) -> String {
        if let Ok(memories) = self.fetch_all().await {
            memories.iter()
                .map(|m| format!("[{}]: {}", m.category, m.value))
                .collect::<Vec<String>>()
                .join("\n")
        } else {
            "No persistent memories established.".to_string()
        }
    }
}
