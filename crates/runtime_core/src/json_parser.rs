use crate::agent_protocol::AgentAction;
use anyhow::Result;

pub fn sanitize_json_escapes(json_str: &str) -> String {
    let mut sanitized = String::with_capacity(json_str.len() + 128);
    let mut in_string = false;
    let mut last_char = ' ';

    for c in json_str.chars() {
        match c {
            '"' if last_char != '\\' => {
                in_string = !in_string;
                sanitized.push('"');
            }
            '\n' if in_string => sanitized.push_str("\\n"),
            '\r' if in_string => sanitized.push_str("\\r"),
            '\t' if in_string => sanitized.push_str("\\t"),
            '\\' => {
                sanitized.push('\\');
            }
            _ => sanitized.push(c),
        }
        last_char = c;
    }
    sanitized
}

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();
    if text.is_empty() || text == "[]" {
        return Ok(vec![]);
    }

    // 1. Raw write artifact intercept
    if text.contains("[WRITE_FILE_TARGET]:") && text.contains("[WRITE_FILE_CONTENT]:") {
        if let Some(target_start) = text.find("[WRITE_FILE_TARGET]:") {
            let path_start = target_start + 20;
            if let Some(path_end) = text[path_start..].find('\n') {
                let path =
                    text[path_start..path_start + path_end].trim().trim_matches('`').to_string();
                if let Some(content_start) = text.find("[WRITE_FILE_CONTENT]:") {
                    let raw_content = &text[content_start + 21..];

                    let content = if let Some(end_tag) = raw_content.find("[/WRITE_FILE_CONTENT]") {
                        raw_content[..end_tag].trim().to_string()
                    } else if let Some(fence_start) = raw_content.find("```") {
                        let after_fence = &raw_content[fence_start + 3..];
                        let final_start = after_fence.find('\n').map(|idx| idx + 1).unwrap_or(0);
                        if let Some(fence_end) = after_fence[final_start..].find("```") {
                            after_fence[final_start..final_start + fence_end].trim().to_string()
                        } else {
                            after_fence[final_start..].trim().to_string()
                        }
                    } else {
                        raw_content.trim().to_string()
                    };

                    if !path.is_empty() && !content.is_empty() {
                        tracing::info!(">> [CORTEX INTERCEPT] Plucked raw artifact for {}", path);
                        return Ok(vec![crate::agent_protocol::AgentAction::WriteFile {
                            path,
                            content,
                            reasoning: "Intercepted via raw text block".to_string(),
                            confidence: 1.0,
                        }]);
                    }
                }
            }
        }
    }

    // 2. Extract markdown code blocks
    let mut extracted = text;
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            extracted = &text[start + 7..start + 7 + end];
        }
    } else if let Some(start) = text.find("```") {
        if let Some(end) = text[start + 3..].find("```") {
            extracted = &text[start + 3..start + 3 + end];
        }
    }

    extracted = extracted.trim();

    // 3. Strict JSON boundary discovery (Do NOT match pseudo-text like [TOOL]:)
    let json_str = if let (Some(s), Some(e)) = (
        extracted
            .find("[{")
            .or_else(|| extracted.find("[\n"))
            .or_else(|| extracted.find("[\""))
            .or_else(|| extracted.find("[ \"")),
        extracted.rfind(']'),
    ) {
        &extracted[s..=e]
    } else if let (Some(s), Some(e)) =
        (extracted.find("{\"").or_else(|| extracted.find("{\n")), extracted.rfind('}'))
    {
        &extracted[s..=e]
    } else {
        return Err(anyhow::anyhow!("No JSON boundaries found in model output."));
    };

    let clean_text = sanitize_json_escapes(json_str);

    if clean_text.contains("00000000000000000000") || clean_text.len() > 35_000 {
        tracing::error!("[CORTEX PARSE ABORT] Token Resonance block. Returning safe halt.");
        return Ok(vec![]);
    }

    let clean_text = if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&clean_text) {
        let process_obj = |obj: &mut serde_json::Map<String, serde_json::Value>| {
            if let Some(tool_val) = obj.remove("tool") {
                obj.insert("type".to_string(), tool_val);
            }
            if let Some(action_val) = obj.remove("action") {
                obj.insert("type".to_string(), action_val);
            }
            if let Some(target_val) = obj.remove("target_path").or_else(|| obj.remove("output")) {
                obj.insert("output_path".to_string(), target_val);
            }
            if let Some(args_val) = obj.remove("args").or_else(|| obj.remove("arguments")) {
                if let Some(args_obj) = args_val.as_object() {
                    for (k, v) in args_obj {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
            if let Some(cmd) = obj.remove("bash_command") {
                obj.insert("command".to_string(), cmd);
            }
        };

        if let Some(arr) = val.as_array_mut() {
            for item in arr.iter_mut() {
                if let Some(obj) = item.as_object_mut() {
                    process_obj(obj);
                }
            }
            val.to_string()
        } else if let Some(obj) = val.as_object_mut() {
            process_obj(obj);
            serde_json::Value::Array(vec![val.clone()]).to_string()
        } else {
            clean_text
        }
    } else {
        clean_text
    };

    match serde_json::from_str::<Vec<AgentAction>>(&clean_text) {
        Ok(actions) => Ok(actions),
        Err(_) => match serde_json::from_str::<AgentAction>(&clean_text) {
            Ok(single) => Ok(vec![single]),
            Err(e) => Err(anyhow::anyhow!("JSON serialization failed: {}", e)),
        },
    }
}
