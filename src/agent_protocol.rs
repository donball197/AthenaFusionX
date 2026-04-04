use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    PatchFile { path: String, search: String, replace: String },
    ExecuteCommand { command: String },
    Message { message: String },
    
    // NEW SOVEREIGN ARCHITECT VOCABULARY
    SystemAudit { target: String, status: String, analysis: Option<serde_json::Value> },
    ComponentStatus { components: Vec<serde_json::Value> },
    ReadinessReport { overall_status: String, version: String, deployment_target: String, next_step: String },
    
    #[serde(other)]
    RejectedAction,
}

impl AgentAction {
    pub fn extract_and_parse(raw_response: &str) -> Result<Vec<AgentAction>, String> {
        let json_str = if let Some(start_idx) = raw_response.find("```json") {
            let body = &raw_response[start_idx + 7..];
            if let Some(end_idx) = body.find("```") {
                &body[..end_idx]
            } else {
                body 
            }
        } else {
            let start = raw_response.find('[');
            let end = raw_response.rfind(']');
            if let (Some(s), Some(e)) = (start, end) {
                if s < e {
                    &raw_response[s..=e]
                } else {
                    return Err("Invalid JSON array boundary.".to_string());
                }
            } else {
                return Err("No JSON array detected in the neural stream.".to_string());
            }
        };

        match serde_json::from_str::<Vec<AgentAction>>(json_str.trim()) {
            Ok(actions) => Ok(actions),
            Err(err) => Err(format!("JSON parsing fault: {}", err)),
        }
    }
}
