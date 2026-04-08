use crate::agent_protocol::AgentAction;
use serde_json::Value;
use regex::Regex;

pub struct NeuralParser;

impl NeuralParser {
    pub fn extract_actions(text: &str) -> anyhow::Result<Vec<AgentAction>> {
        let mut actions = Vec::new();
        let thought_re = Regex::new(r"(?s)<\|(?:think|thought|channel)>.*?<\|(?:end|channel)\|?>").unwrap();
        let stripped_text = thought_re.replace_all(text, "");
        let unescaped_text = stripped_text.replace("&quot;", "\"").replace("&amp;", "&");
        
        if let (Some(start), Some(end)) = (unescaped_text.find('['), unescaped_text.rfind(']')) {
            if start < end {
                let array_str = &unescaped_text[start..=end];
                if let Ok(batch) = serde_json::from_str::<Vec<AgentAction>>(array_str) {
                    return Ok(batch);
                }
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
            Err(anyhow::anyhow!("DNA_TRANSLATION_FAILURE: Bridge could not bind actions to schema."))
        } else {
            Ok(actions)
        }
    }
}
