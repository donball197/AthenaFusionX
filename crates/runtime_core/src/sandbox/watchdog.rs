//! Temporal watchdog with cooperative cancellation.
//! WARNING: The inner closure MUST check the CancellationToken periodically.
//! Non-cooperative tasks may leak threads.

use tokio_util::sync::CancellationToken;
use std::time::Duration;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum WatchdogError {
    #[error("Execution timed out after {0:?}")]
    Timeout(Duration),
    #[error("Task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Clone)]
pub struct ExecutionWatchdog {
    timeout: Duration,
}

impl ExecutionWatchdog {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    pub async fn monitor<F, R>(&self, f: F) -> Result<R, WatchdogError>
    where
        F: FnOnce(CancellationToken) -> R + Send + 'static,
        R: Send + 'static,
    {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let handle = tokio::task::spawn_blocking(move || {
            f(token_clone)
        });

        tokio::select! {
            res = handle => {
                token.cancel();
                res.map_err(WatchdogError::Join)
            }
            _ = tokio::time::sleep(self.timeout) => {
                token.cancel();
                error!(
                    "ExecutionWatchdog timeout after {:?}. Task may still be running – ensure cooperative cancellation.",
                    self.timeout
                );
                Err(WatchdogError::Timeout(self.timeout))
            }
        }
    }
}
