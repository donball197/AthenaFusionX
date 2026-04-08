use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    #[serde(alias = "ReadFile")] ReadFile { path: String },
    #[serde(alias = "WriteFile")] WriteFile { path: String, content: String },
    #[serde(alias = "PatchFile")] PatchFile { path: String, search: String, replace: String },
    #[serde(alias = "ExecuteCommand")] ExecuteCommand { command: String },
    #[serde(alias = "Message")] Message { message: String },
    
    #[serde(alias = "GenerateReport")]
    GenerateReport { 
        #[serde(flatten)] data: Value 
    },
    
    // THE LIQUID CATCH-ALL
    #[serde(other)]
    SovereignDirective,
}

impl AgentAction {
    pub fn extract_and_parse(raw_response: &str) -> Result<Vec<AgentAction>, String> {
        let unescaped = raw_response.replace("&quot;", "\"");
        
        let start = unescaped.find("[{").or_else(|| unescaped.find("[ {"));
        let end = unescaped.rfind(']');
        
        let json_str = match (start, end) {
            (Some(s), Some(e)) if s < e => &unescaped[s..=e],
            _ => return Err("No actionable logic array found.".into()),
        };

        // 1. Parse into a raw Value array
        let val_array: Vec<Value> = serde_json::from_str(json_str.trim())
            .map_err(|e| format!("JSON syntax error: {}", e))?;

        // 2. Liquid Hoisting & Shape-Shifting
        let mut actions = Vec::new();
        
        for mut val in val_array {
            if let Some(obj) = val.as_object_mut() {
                
                // SUTURE: Detect and repair External Tagging
                // Converts {"Message": {"message": "..."}} -> {"action": "Message", "message": "..."}
                let keys: Vec<String> = obj.keys().cloned().collect();
                if keys.len() == 1 && keys[0] != "action" {
                    let variant_name = keys[0].clone();
                    if let Some(inner_val) = obj.remove(&variant_name) {
                        if let Some(inner_obj) = inner_val.as_object() {
                            for (k, v) in inner_obj { obj.insert(k.clone(), v.clone()); }
                        } else {
                            // If the payload isn't an object, shove it into data
                            obj.insert("data".to_string(), inner_val);
                        }
                        obj.insert("action".to_string(), Value::String(variant_name));
                    }
                }

                // Flatten common nesting keys (including 'data' if the AI wrapped it unnecessarily)
                for key in ["parameters", "args", "details", "data"] {
                    if let Some(nested) = obj.remove(key) {
                        if let Some(n_obj) = nested.as_object() {
                            for (k, v) in n_obj { obj.insert(k.clone(), v.clone()); }
                        }
                    }
                }
                
                // Route unrecognized actions to the catch-all
                let action_name = obj.get("action").and_then(|a| a.as_str()).map(|s| s.to_string());
                if let Some(name) = action_name {
                    let core_actions = [
                        "read_file", "ReadFile", 
                        "write_file", "WriteFile", 
                        "patch_file", "PatchFile",
                        "execute_command", "ExecuteCommand", 
                        "message", "Message",
                        "generate_report", "GenerateReport"
                    ];
                    
                    if !core_actions.contains(&name.as_str()) {
                        obj.insert("action".to_string(), serde_json::json!("generate_report"));
                        obj.insert("original_action_name".to_string(), serde_json::json!(name));
                    }
                }
            }
            
            // 3. Attempt mapping
            match serde_json::from_value::<AgentAction>(val.clone()) {
                Ok(action) => actions.push(action),
                Err(e) => println!("\x1b[33m[SUTURE WARNING]: Failed to map fragment: {}\x1b[0m", e),
            }
        }

        if actions.is_empty() {
            Err("Failed to parse any valid actions from the stream.".into())
        } else {
            Ok(actions)
        }
    }
}
