use crate::agent_protocol::AgentAction;
use crate::nexus_error::NexusError;
use serde_json::Value;

pub struct NeuralParser;

impl NeuralParser {
    /// extract_actions: Extracts neural intent and utilizes the 
    /// fuzzy deserializer in agent_protocol.rs to map hardware actions.
    pub fn extract_actions(text: &str) -> Result<Vec<AgentAction>, NexusError> {
        let clean = text.replace("```json", "").replace("```", "").trim().to_string();
        
        let mut actions = Vec::new();
        
        // Find the outermost structure
        // Aggressively locate the JSON array boundaries
        let start = clean.find('[');
        let end = clean.rfind(']');
        
        // Fallback to object boundaries if it didn't output an array
        let (start, end) = if start.is_some() && end.is_some() {
            (start, end)
        } else {
            (clean.find('{'), clean.rfind('}'))
        };

        if let (Some(s), Some(e)) = (start, end) {
            let json_slice = &clean[s..=e];
            let val: Value = serde_json::from_str(json_slice)
                .map_err(|err| NexusError::Other(format!("JSON Structure Fault: {}", err)))?;

            if val.is_array() {
                // Uses the custom Deserialize impl in agent_protocol.rs for each item
                if let Ok(batch) = serde_json::from_value::<Vec<AgentAction>>(val) {
                    actions = batch;
                }
            } else {
                // Uses the custom Deserialize impl for the single object
                if let Ok(single) = serde_json::from_value::<AgentAction>(val) {
                    actions.push(single);
                }
            }
        }

        if actions.is_empty() {
            Err(NexusError::Other("DNA_TRANSLATION_FAILURE: No valid actions found".into()))
        } else {
            Ok(actions)
        }
    }
}
