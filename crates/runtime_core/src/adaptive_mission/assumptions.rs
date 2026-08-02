use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssumptionError {
    #[error("Assumption not found: {0}")]
    NotFound(String),
    #[error("Validation failed for assumption {0}: {1}")]
    ValidationFailed(String, String),
    #[error("Precondition check failed: {0}")]
    PreconditionFailed(String),
    #[error("Postcondition check failed: {0}")]
    PostconditionFailed(String),
}

#[derive(Debug, Clone)]
pub struct Assumption {
    pub id: String,
    pub description: String,
    // Add additional fields as needed for the Gemma 3 API routing
}

// Placeholder for DAGNodeAssumption to satisfy mod.rs exports
#[derive(Debug, Clone)]
pub struct DAGNodeAssumption {
    pub node_id: String,
    pub assumption: Assumption,
}
