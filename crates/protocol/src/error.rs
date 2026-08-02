use thiserror::Error;

#[derive(Error, Debug)]
pub enum NexusError {
    #[error("Execution fault: {0}")]
    Execution(String),
    #[error("IO fault: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration fault: {0}")]
    Config(String),
    #[error("Orchestration fault: {0}")]
    Orchestration(String),
    #[error("Internal system fault: {0}")]
    Internal(String),
    #[error("Sandbox execution fault: {0}")]
    Sandbox(String),
    #[error("Legacy conversion fault: {0}")]
    Legacy(#[from] anyhow::Error),
}

pub type NexusResult<T> = Result<T, NexusError>;
