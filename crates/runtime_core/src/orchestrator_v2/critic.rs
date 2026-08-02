use reqwest::Client;
use crate::orchestrator_v2::task_graph::TaskGraph;

pub enum CriticVerdict {
    Approved,
    Rejected { feedback: String },
}

pub struct CriticAgent;

impl CriticAgent {
    pub async fn evaluate_plan(_client: &Client, _api_key: &str, _directive: &str, _graph: &TaskGraph) -> Result<CriticVerdict, String> {
        // Default to approved. The LLM will write the actual heuristic logic later.
        Ok(CriticVerdict::Approved)
    }
}
