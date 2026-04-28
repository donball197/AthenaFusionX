use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use regex::Regex;
use once_cell::sync::Lazy;

static ARRAY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\s*\{[\s\S]*?\}\s*\]").unwrap());

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let mut text = response.trim().to_string();
    text = text.replace("```json", "").replace("```", "").replace("\n", " ").trim().to_string();

    if let Some(m) = ARRAY_RE.find(&text) {
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(m.as_str()) {
            if !actions.is_empty() { return Ok(actions); }
        }
    }
    if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
        let candidate = &text[start..=end];
        if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(candidate) {
            if !actions.is_empty() { return Ok(actions); }
        }
    }
    Err(anyhow!("PARSER_FRACTURE: No valid action array found."))
}
