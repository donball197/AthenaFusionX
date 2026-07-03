use reqwest::Client;

pub struct GraphOfThoughts;

impl GraphOfThoughts {
    pub async fn explore_strategies(_client: &Client, _api_key: &str, directive: &str) -> String {
        // Pass-through for now, wiring the structural boundary
        directive.to_string()
    }
}
