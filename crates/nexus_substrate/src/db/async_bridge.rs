//! Async bridge for SQLite using parking_lot::Mutex (no poisoning).

use parking_lot::Mutex;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub async fn db_exec<F, R>(
    conn: Arc<Mutex<rusqlite::Connection>>,
    f: F,
) -> Result<R, DbError>
where
    F: FnOnce(&rusqlite::Connection) -> Result<R, rusqlite::Error> + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock();
        f(&guard).map_err(DbError::Sqlite)
    })
    .await
    .map_err(DbError::Join)?
}

pub async fn db_exec_mut<F, R>(
    conn: Arc<Mutex<rusqlite::Connection>>,
    f: F,
) -> Result<R, DbError>
where
    F: FnOnce(&mut rusqlite::Connection) -> Result<R, rusqlite::Error> + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let mut guard = conn.lock();
        f(&mut guard).map_err(DbError::Sqlite)
    })
    .await
    .map_err(DbError::Join)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[tokio::test]
    async fn test_db_exec() {
        let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        let result = db_exec(conn.clone(), |c| {
            c.execute("CREATE TABLE test (id INTEGER)", [])?;
            Ok(())
        }).await;
        assert!(result.is_ok());
    }
}
