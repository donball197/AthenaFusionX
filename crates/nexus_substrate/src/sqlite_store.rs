use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(path: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}", path))
            .await?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous=NORMAL;").execute(&pool).await?;

        sqlx::query(
            "
            CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                ts DATETIME DEFAULT CURRENT_TIMESTAMP
            );
        ",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "
            CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                payload BLOB NOT NULL
            );
        ",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn append_event(&self, kind: &str, object_id: &str) -> Result<()> {
        sqlx::query("INSERT INTO events (kind, object_id) VALUES (?, ?)")
            .bind(kind)
            .bind(object_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
