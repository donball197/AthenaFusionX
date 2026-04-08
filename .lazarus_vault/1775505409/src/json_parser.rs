use crate::agent_protocol::AgentAction;
use crate::nexus_error::NexusError;
use serde_json::Value;
use regex::Regex;

pub struct NeuralParser;

impl NeuralParser {
    pub fn extract_actions(text: &str) -> Result<Vec<AgentAction>, NexusError> {
        let mut actions = Vec::new();
        
        // 1. Remove Sovereign Thought Blocks (<|think|>, <|thought|>, <|channel|>)
        let thought_re = Regex::new(r"(?s)<\|(?:think|thought|channel)>.*?<\|(?:end|channel)\|?>").unwrap();
        let stripped_text = thought_re.replace_all(text, "");
        
        // 2. Revert HTML entities
        let unescaped_text = stripped_text.replace("&quot;", "\"").replace("&amp;", "&");
        
        // 3. Locate the outermost array [ ... ]
        if let (Some(start), Some(end)) = (unescaped_text.find('['), unescaped_text.rfind(']')) {
            if start < end {
                let array_str = &unescaped_text[start..=end];
                
                // Attempt direct array parsing
                if let Ok(batch) = serde_json::from_str::<Vec<AgentAction>>(array_str) {
                    return Ok(batch);
                }
                
                // Fallback: Try parsing as generic Value array to filter valid actions
                if let Ok(val) = serde_json::from_str::<Value>(array_str) {
                    if let Some(arr) = val.as_array() {
                        for item in arr {
                            if let Ok(action) = serde_json::from_value::<AgentAction>(item.clone()) {
                                actions.push(action);
                            }
                        }
                    }
                }
            }
        }
        
        if actions.is_empty() {
            Err(NexusError::Other("DNA_TRANSLATION_FAILURE: Bridge could not bind actions to schema.".into()))
        } else {
            Ok(actions)
        }
    }
}
