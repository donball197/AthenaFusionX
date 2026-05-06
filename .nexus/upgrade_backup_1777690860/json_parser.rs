use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;
use serde_json::{json, Value};

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let stripped = response.trim()
        .replace("```json", "")
        .replace("```", "");

    let renamed = stripped
        .replace("\"action\":", "\"type\":")
        .replace("\"action\" :", "\"type\":")
        .replace("\"file_path\":", "\"path\":")
        .replace("\"fn_name\":", "\"function_name\":");

    let start = renamed.find('[')
        .ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array start '[' found"))?;
    let end = renamed.rfind(']')
        .ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array end ']' found"))?;

    let json_text = &renamed[start..=end];

    if let Ok(mut parsed_json) = serde_json::from_str::<Value>(json_text) {
        if let Some(arr) = parsed_json.as_array_mut() {
            for obj in arr.iter_mut() {
                // ⚡ FIX 1: Dynamically flatten nested JSON like {"read_file": {"path": "..."}}
                let mut flattened = None;
                if let Some(map) = obj.as_object() {
                    if map.len() == 1 && !map.contains_key("type") {
                        let (k, v) = map.into_iter().next().unwrap();
                        if let Some(mut inner) = v.as_object().cloned() {
                            inner.insert("type".to_string(), json!(k));
                            flattened = Some(Value::Object(inner));
                        }
                    }
                }
                if let Some(flat) = flattened {
                    *obj = flat;
                }

                // Normal inference fallback
                if let Some(map) = obj.as_object_mut() {
                    if !map.contains_key("type") {
                        if map.contains_key("search") && map.contains_key("replace") {
                            map.insert("type".to_string(), json!("diff_patch"));
                        } else if map.contains_key("command") {
                            map.insert("type".to_string(), json!("execute_command"));
                        } else if map.contains_key("path") && map.contains_key("content") {
                            map.insert("type".to_string(), json!("write_file"));
                        } else if map.contains_key("path") {
                            map.insert("type".to_string(), json!("read_file"));
                        }
                    }
                }
            }
        }

        if let Ok(actions) = serde_json::from_value::<Vec<AgentAction>>(parsed_json) {
            if !actions.is_empty() { return Ok(actions); }
        }
    }

    Err(anyhow!("PARSER_FRACTURE: Malformed or unrecoverable array"))
}
