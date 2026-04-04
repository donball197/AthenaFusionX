use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    PatchFile { path: String, search: String, replace: String },
    ExecuteCommand { command: String },
    Message { message: String },
    #[serde(other)]
    RejectedAction,
}

impl AgentAction {
    /// Surgical JSON Extractor: Strips markdown and isolates the JSON array
    pub fn extract_and_parse(raw_response: &str) -> Result<Vec<AgentAction>, String> {
        let start = raw_response.find('[');
        let end = raw_response.rfind(']');
        
        if let (Some(s), Some(e)) = (start, end) {
            if s < e {
                let json_slice = &raw_response[s..=e];
                match serde_json::from_str::<Vec<AgentAction>>(json_slice) {
                    Ok(actions) => Ok(actions),
                    Err(err) => Err(format!("JSON parsing fault: {}", err)),
                }
            } else {
                Err("Invalid JSON array boundary.".to_string())
            }
        } else {
            Err("No JSON array detected in the neural stream.".to_string())
        }
    }
}
