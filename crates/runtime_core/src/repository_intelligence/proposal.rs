/// Represents a structured proposal within the repository intelligence system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Unique identifier for the author of the proposal.
    pub author: String,
    /// Detailed text content of the proposal.
    pub content: String,
    /// Current accumulated score or vote count for the proposal.
    pub votes: i32,
}
