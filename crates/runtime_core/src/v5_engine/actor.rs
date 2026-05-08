use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;
use tokio::sync::{mpsc, oneshot};

pub enum MemoryCommand {
    CommitEvent {
        kind: String,
        object_id: String,
        respond_to: oneshot::Sender<bool>,
    },
    CommitArtifact {
        task_id: String,
        artifact_type: String,
        payload: Vec<u8>,
        respond_to: oneshot::Sender<bool>,
    },
}

pub struct MemoryActor {
    rx: mpsc::Receiver<MemoryCommand>,
    pool: SqlitePool,
}

impl MemoryActor {
    pub async fn ignite(db_url: &str) -> Result<mpsc::Sender<MemoryCommand>, sqlx::Error> {
        // Enforce WAL mode for ARM64 IO performance
        let options = SqliteConnectOptions::from_str(db_url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePool::connect_with(options).await?;

        // MSK Core Schema
        sqlx::query(
            "
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                objective TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                ts INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                artifact_type TEXT NOT NULL,
                payload BLOB NOT NULL
            );
        ",
        )
        .execute(&pool)
        .await?;

        // Bound the queue to 64 to enforce backpressure
        let (tx, rx) = mpsc::channel(64);

        let actor = Self { rx, pool };

        tokio::spawn(async move {
            actor.run_loop().await;
        });

        println!(
            "\x1b[32m>> [ACTOR] MSK SQLite WAL Substrate Online. Single-writer active.\x1b[0m"
        );
        Ok(tx)
    }

    async fn run_loop(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                MemoryCommand::CommitEvent { kind, object_id, respond_to } => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let res =
                        sqlx::query("INSERT INTO events (kind, object_id, ts) VALUES (?, ?, ?)")
                            .bind(kind)
                            .bind(object_id)
                            .bind(ts)
                            .execute(&self.pool)
                            .await
                            .is_ok();
                    let _ = respond_to.send(res);
                }
                MemoryCommand::CommitArtifact { task_id, artifact_type, payload, respond_to } => {
                    let id = format!("{}_{}", task_id, artifact_type); // Simple compound ID
                    let res = sqlx::query("INSERT INTO artifacts (id, task_id, artifact_type, payload) VALUES (?, ?, ?, ?)")
                        .bind(id).bind(task_id).bind(artifact_type).bind(payload)
                        .execute(&self.pool).await.is_ok();
                    let _ = respond_to.send(res);
                }
            }
        }
    }
}
