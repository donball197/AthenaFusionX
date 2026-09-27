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

    // 2. Extract markdown code blocks -- but only when the payload is
    //    NOT already a bare JSON array/object at the top level.
    let mut extracted = text;
    let is_bare_json = text.starts_with('[') || text.starts_with('{');
    if !is_bare_json {
        if let Some(start) = text.find("```json") {
            if let Some(end) = text[start + 7..].find("```") {
                extracted = &text[start + 7..start + 7 + end];
            }
        } else if let Some(start) = text.find("```") {
            if let Some(end) = text[start + 3..].find("```") {
                extracted = &text[start + 3..start + 3 + end];
            }
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

            // The native ReadFileChunk contract uses `max_lines`, but some
            // models emit an inclusive `end_line`. Normalize that model-side
            // alias before strict AgentAction deserialization.
            let is_read_file_chunk = obj
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|action_type| action_type == "read_file_chunk");

            if is_read_file_chunk {
                // filepath is a model-side alias for the native `path`
                // field. Normalize it only for read_file_chunk so unrelated
                // action schemas are not changed. Explicit `path` wins.
                // filepath alias normalization
                if !obj.contains_key("path") {
                    if let Some(filepath) = obj.remove("filepath") {
                        obj.insert("path".to_string(), filepath);
                    }
                }

                if !obj.contains_key("start_line") {
                    if let Some(offset) = obj.remove("offset") {
                        obj.insert("start_line".to_string(), offset);
                    }
                }
                if !obj.contains_key("max_lines") {
                    if let Some(limit) = obj.remove("limit") {
                        obj.insert("max_lines".to_string(), limit);
                    }
                }
                if !obj.contains_key("max_lines") {
                    if let (Some(start_line), Some(end_line)) = (
                        obj.get("start_line").and_then(serde_json::Value::as_u64),
                        obj.get("end_line").and_then(serde_json::Value::as_u64),
                    ) {
                        if end_line >= start_line {
                            obj.insert(
                                "max_lines".to_string(),
                                serde_json::Value::from(end_line - start_line + 1),
                            );
                        }
                    }
                }
                obj.remove("end_line");
            }

            // Nested Envelope Unwrapper for write_file
            let is_write_file = obj
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|action_type| action_type == "write_file");

            if is_write_file {
                let content_owned: Option<String> =
                    obj.get("content").and_then(serde_json::Value::as_str).map(str::to_string);

                if let Some(content_val) = content_owned {
                    let trimmed = content_val.trim();

                    let looks_structured = trimmed.starts_with("```json")
                        || trimmed.starts_with("```")
                        || trimmed.starts_with('[')
                        || trimmed.starts_with('{');

                    if looks_structured
                        && trimmed.contains(r#""type""#)
                        && trimmed.contains(r#""write_file""#)
                        && trimmed.contains(r#""content""#)
                    {
                        let candidate =
                            if trimmed.starts_with("```json") && trimmed.ends_with("```") {
                                trimmed["```json".len()..trimmed.len() - 3].trim()
                            } else if trimmed.starts_with("```") && trimmed.ends_with("```") {
                                trimmed[3..trimmed.len() - 3].trim()
                            } else {
                                trimmed
                            };

                        if let Ok(nested_val) = serde_json::from_str::<serde_json::Value>(candidate)
                        {
                            let inner_content = nested_val
                                .as_array()
                                .and_then(|arr| arr.first())
                                .and_then(|item| item.as_object())
                                .and_then(|item| item.get("type"))
                                .and_then(serde_json::Value::as_str)
                                .filter(|kind| *kind == "write_file")
                                .and_then(|_| {
                                    nested_val
                                        .as_array()
                                        .and_then(|arr| arr.first())
                                        .and_then(|item| item.get("content"))
                                        .and_then(serde_json::Value::as_str)
                                })
                                .or_else(|| {
                                    nested_val
                                        .as_object()
                                        .filter(|item| {
                                            item.get("type")
                                                .and_then(serde_json::Value::as_str)
                                                .is_some_and(|kind| kind == "write_file")
                                        })
                                        .and_then(|item| item.get("content"))
                                        .and_then(serde_json::Value::as_str)
                                });

                            if let Some(real_text) = inner_content {
                                obj.insert(
                                    "content".to_string(),
                                    serde_json::Value::String(real_text.to_string()),
                                );
                            }
                        }
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::extract_actions;
    use crate::agent_protocol::AgentAction;

    fn extract_single(json: &str) -> AgentAction {
        let actions = extract_actions(json).expect("extract_actions should succeed");
        assert_eq!(actions.len(), 1, "expected exactly one action");
        actions.into_iter().next().unwrap()
    }

    #[test]
    fn read_file_chunk_accepts_native_max_lines() {
        let action = extract_single(
            r#"[{"type":"read_file_chunk","path":"src/lib.rs","start_line":1,"max_lines":50,"reasoning":"native form"}]"#,
        );
        match action {
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => {
                assert_eq!(path, "src/lib.rs");
                assert_eq!(start_line, 1);
                assert_eq!(max_lines, 50);
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }

    #[test]
    fn read_file_chunk_normalizes_end_line_alias() {
        // The exact shape that caused the 2026-09-13 audit spin.
        // Inclusive end_line: 52..=100 => 49 lines.
        let action = extract_single(
            r#"[{"type":"read_file_chunk","path":"crates/runtime_core/src/cortical_loop.rs","start_line":52,"end_line":100,"reasoning":"model emitted inclusive range"}]"#,
        );
        match action {
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => {
                assert!(path.ends_with("cortical_loop.rs"));
                assert_eq!(start_line, 52);
                assert_eq!(max_lines, 49, "end_line 100 inclusive - start 52 + 1");
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }

    #[test]
    fn read_file_chunk_normalizes_filepath_alias() {
        // Regression for the observed model payload:
        // `filepath` must normalize to the native `path` field, while the
        // existing inclusive `end_line` normalization remains intact.
        let action = extract_single(
            r#"[{"type":"read_file_chunk","filepath":"crates/runtime_core/src/cortical_loop.rs","start_line":150,"end_line":200,"reasoning":"model emitted filepath alias"}]"#,
        );

        match action {
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => {
                assert!(path.ends_with("cortical_loop.rs"));
                assert_eq!(start_line, 150);
                assert_eq!(max_lines, 51, "end_line 200 inclusive - start 150 + 1");
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }

    #[test]
    fn read_file_chunk_single_line_range() {
        // start_line == end_line => max_lines == 1
        let action = extract_single(
            r#"[{"type":"read_file_chunk","path":"a.rs","start_line":7,"end_line":7,"reasoning":""}]"#,
        );
        match action {
            AgentAction::ReadFileChunk { start_line, max_lines, .. } => {
                assert_eq!(start_line, 7);
                assert_eq!(max_lines, 1);
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }

    #[test]
    fn read_file_chunk_rejects_reversed_range() {
        // end_line < start_line: max_lines is not derived; serde fails on
        // missing max_lines and extract_actions returns Err. That is the
        // correct behavior -- a reversed range is a malformed proposal, and
        // the cortical loop's recovery path handles the fault.
        let result = extract_actions(
            r#"[{"type":"read_file_chunk","path":"a.rs","start_line":100,"end_line":50,"reasoning":""}]"#,
        );
        assert!(result.is_err(), "reversed range must not silently produce max_lines");
    }

    #[test]
    fn replace_lines_end_line_is_untouched() {
        // ReplaceLines legitimately uses start_line + end_line; the
        // read_file_chunk normalizer must not strip end_line from it.
        let action = extract_single(
            r#"[{"type":"replace_lines","path":"a.rs","start_line":10,"end_line":20,"replacement":"new body","reasoning":""}]"#,
        );
        match action {
            AgentAction::ReplaceLines { start_line, end_line, .. } => {
                assert_eq!(start_line, 10);
                assert_eq!(end_line, 20);
            }
            other => panic!("expected ReplaceLines, got {:?}", other),
        }
    }

    #[test]
    fn bare_outer_array_with_inner_fence_parses() {
        let payload = r##"[{"type":"write_file","path":"report.md","content":"```json\n[\n  {\n    \"type\": \"write_file\",\n    \"path\": \"report.md\",\n    \"content\": \"# Real Report Content\"\n  }\n]\n```","reasoning":"test"}]"##;
        let action = extract_single(payload);
        match action {
            AgentAction::WriteFile { path, content, .. } => {
                assert_eq!(path, "report.md");
                assert_eq!(content, "# Real Report Content");
            }
            other => panic!("expected WriteFile, got {:?}", other),
        }
    }

    #[test]
    fn bare_outer_object_with_nested_object_envelope_unwraps() {
        let payload = r###"{"type":"write_file","path":"summary.md","content":"{\"type\":\"write_file\",\"path\":\"summary.md\",\"content\":\"## Clean Architecture Summary\"}","reasoning":"test"}"###;
        let action = extract_single(payload);
        match action {
            AgentAction::WriteFile { path, content, .. } => {
                assert_eq!(path, "summary.md");
                assert_eq!(content, "## Clean Architecture Summary");
            }
            other => panic!("expected WriteFile, got {:?}", other),
        }
    }

    #[test]
    fn read_file_chunk_accepts_offset_and_limit() {
        let action = extract_single(
            r#"[{"type":"read_file_chunk","path":"src/lib.rs","offset":100,"limit":50,"reasoning":"pagination aliases"}]"#,
        );
        match action {
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => {
                assert_eq!(path, "src/lib.rs");
                assert_eq!(start_line, 100);
                assert_eq!(max_lines, 50);
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }
}
