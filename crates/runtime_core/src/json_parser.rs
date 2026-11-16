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

/// Fix 2: unwrap common "plan then execute" and function-call envelopes
/// emitted by the strategist model before handing the payload to
/// `extract_actions`. Fires only when the envelope shape parses AND the
/// unwrapped `Vec<AgentAction>` deserializes cleanly, so a well-shaped
/// action array is unaffected.
///
/// Handles:
///   {"reasoning": "...", "actions": [{...}, ...]}     -- plan-then-execute
///   {"tool": "<name>", "args": {<fields>}}            -- function-call
///
/// Both may be bare or wrapped in ```json fences.
fn try_unwrap_envelope(s: &str) -> Option<Vec<crate::agent_protocol::AgentAction>> {
    let stripped =
        s.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let candidates: [&str; 2] = [s, stripped];
    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) else {
            continue;
        };
        // Envelope A: {"reasoning": "...", "actions": [...]}
        if let Some(arr) = val.get("actions").and_then(|v| v.as_array()) {
            if let Ok(parsed) = serde_json::from_value::<Vec<crate::agent_protocol::AgentAction>>(
                serde_json::Value::Array(arr.clone()),
            ) {
                return Some(parsed);
            }
        }
        // Envelope B: {"tool": "<name>", "args": {<fields>}}
        if let Some(tool) = val.get("tool").and_then(|v| v.as_str()) {
            if let Some(args) = val.get("args").and_then(|v| v.as_object()) {
                let mut action_obj = args.clone();
                action_obj.insert("type".into(), serde_json::Value::String(tool.to_string()));
                // Common field-name confusion: models often emit `filepath`
                // when the schema expects `path`. Alias it here so the
                // function-call envelope round-trips through AgentAction.
                if action_obj.contains_key("filepath") && !action_obj.contains_key("path") {
                    let filepath = action_obj.remove("filepath").unwrap();
                    action_obj.insert("path".into(), filepath);
                }
                let wrapped = serde_json::Value::Array(vec![serde_json::Value::Object(action_obj)]);
                if let Ok(parsed) =
                    serde_json::from_value::<Vec<crate::agent_protocol::AgentAction>>(wrapped)
                {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    let text = response.trim();
    if text.is_empty() || text == "[]" {
        return Ok(vec![]);
    }

    // Fix 2: Envelope unwrap for common "plan then execute" shapes.
    // Handles {"reasoning": ..., "actions": [...]} and {"tool": ..., "args": {...}}
    // before any boundary discovery, so a well-shaped action array is untouched.
    if let Some(actions) = try_unwrap_envelope(text) {
        return Ok(actions);
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
                            _thought_process: None,
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

    // 3. Strict JSON boundary discovery (Do NOT match pseudo-text like [TOOL]:
    //    or marker brackets like `[READ_OK]` inside a top-level `{...}` object's string fields).
    let arr_start = extracted
        .find("[{")
        .or_else(|| extracted.find("[\n"))
        .or_else(|| extracted.find("[\""))
        .or_else(|| extracted.find("[ \""))
        .or_else(|| extracted.find('['));
    let obj_start = extracted.find("{\"").or_else(|| extracted.find("{\n"));

    let json_str = match (extracted.starts_with('{'), obj_start, arr_start) {
        (true, _, _) => match (extracted.find('{'), extracted.rfind('}')) {
            (Some(s), Some(e)) if e >= s => &extracted[s..=e],
            _ => return Err(anyhow::anyhow!("No JSON boundaries found in model output.")),
        },
        (false, Some(os), Some(as_)) if os < as_ => match extracted.rfind('}') {
            Some(e) if e >= os => &extracted[os..=e],
            _ => return Err(anyhow::anyhow!("No JSON boundaries found in model output.")),
        },
        (false, Some(os), None) => match extracted.rfind('}') {
            Some(e) if e >= os => &extracted[os..=e],
            _ => return Err(anyhow::anyhow!("No JSON boundaries found in model output.")),
        },
        (false, _, Some(as_)) => match extracted.rfind(']') {
            Some(e) if e >= as_ => &extracted[as_..=e],
            _ => return Err(anyhow::anyhow!("No JSON boundaries found in model output.")),
        },
        _ => return Err(anyhow::anyhow!("No JSON boundaries found in model output.")),
    };

    // Models sometimes emit several JSON values back to back (`{..}{..}` or
    // `[..]\n[..]`). Keep only the first complete value; if it does not parse,
    // fall back to the unmodified text so existing behavior is unchanged.
    fn first_json_value(s: &str) -> String {
        let mut it = serde_json::Deserializer::from_str(s).into_iter::<serde_json::Value>();
        match it.next() {
            Some(Ok(_)) => {
                let end = it.byte_offset();
                s[..end].to_string()
            }
            _ => s.to_string(),
        }
    }
    let clean_text = sanitize_json_escapes(json_str);
    let clean_text: String = first_json_value(&clean_text.to_string());

    if clean_text.contains("00000000000000000000") || clean_text.len() > 35_000 {
        tracing::error!("[CORTEX PARSE ABORT] Token Resonance block. Returning safe halt.");
        return Ok(vec![]);
    }

    let clean_text = if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&clean_text) {
        let process_obj = |obj: &mut serde_json::Map<String, serde_json::Value>| {
            // Float to Int normalization for model quirks
            for key in ["start_line", "max_lines", "end_line", "offset", "limit"] {
                if let Some(val) = obj.get_mut(key) {
                    if let Some(s) = val.as_str() {
                        if let Ok(f) = s.parse::<f64>() {
                            *val = serde_json::Value::from(f as u64);
                        }
                    } else if let Some(f) = val.as_f64() {
                        *val = serde_json::Value::from(f as u64);
                    }
                }
            }

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
            // ReAct-style nesting: {"action": "x", "action_input": {...}}. Hoist the inner
            // fields to the top level; explicit top-level keys win.
            for nested_key in ["action_input", "input", "parameters", "params"] {
                if let Some(inner) = obj.remove(nested_key) {
                    if let Some(inner_obj) = inner.as_object() {
                        for (k, v) in inner_obj {
                            obj.entry(k.clone()).or_insert_with(|| v.clone());
                        }
                    }
                }
            }

            // {"halt": true, "reasoning": ...} and {"halt": {...}} shapes.
            if !obj.contains_key("type") {
                if let Some(halt_val) = obj.remove("halt") {
                    obj.insert("type".to_string(), serde_json::Value::String("halt".to_string()));
                    if let Some(inner_obj) = halt_val.as_object() {
                        for (k, v) in inner_obj {
                            obj.entry(k.clone()).or_insert_with(|| v.clone());
                        }
                    }
                }
            }

            // Halt reasoning given as an object of labeled sections: flatten to text.
            if obj.get("type").and_then(serde_json::Value::as_str) == Some("halt") {
                if let Some(sections) =
                    obj.get("reasoning").and_then(serde_json::Value::as_object).cloned()
                {
                    let text = sections
                        .iter()
                        .map(|(k, v)| {
                            format!(
                                "{}: {}",
                                k,
                                v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    obj.insert("reasoning".to_string(), serde_json::Value::String(text));
                }
            }

            if let Some(cmd) = obj.remove("bash_command") {
                obj.insert("command".to_string(), cmd);
            }

            // The native ReadFileChunk contract uses `max_lines`, but some
            // models emit an inclusive `end_line`. Normalize that model-side
            // alias before strict AgentAction deserialization.
            // filepath is a common model-side alias for `path`. Normalize when
            // explicit `path` is absent.
            if !obj.contains_key("path") {
                if let Some(filepath) = obj.remove("filepath") {
                    obj.insert("path".to_string(), filepath);
                }
            }

            let is_read_file_chunk = obj
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|action_type| action_type == "read_file_chunk");

            if is_read_file_chunk {
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

            //             // Normalize replace_lines model-side field names
            //             let is_replace_lines = obj
            //                 .get("type")
            //                 .and_then(serde_json::Value::as_str)
            //                 .is_some_and(|t| t == "replace_lines");
            //             if is_replace_lines {
            //                 if !obj.contains_key("replacement") {
            //                     if let Some(v) = obj
            //                         .remove("new_content")
            //                         .or_else(|| obj.remove("content"))
            //                         .or_else(|| obj.remove("new_body"))
            //                     {
            //                         obj.insert("replacement".to_string(), v);
            //                     }
            //                 }
            //             }
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

    // Strip model control-token leaks (<|end|>, <rfl:...>) from string fields.
    // A halt whose text was cut is a parse fault: re-propose, do not accept.
    let clean_text = match serde_json::from_str::<serde_json::Value>(&clean_text) {
        Ok(orig) => {
            let confined = crate::proposal_sanitize::leak_confined_to_reasoning(&orig);
            let s = crate::proposal_sanitize::sanitize(orig);
            if s.stripped && !confined {
                return Err(anyhow::anyhow!(
                    "PARSE_FAULT: control-token leak in proposal; re-propose"
                ));
            }
            serde_json::to_string(&s.value).unwrap_or(clean_text)
        }
        Err(_) => clean_text,
    };

    match serde_json::from_str::<Vec<AgentAction>>(&clean_text) {
        Ok(actions) => Ok(actions),
        Err(vec_err) => {
            if clean_text.trim_start().starts_with('[') {
                return Err(anyhow::anyhow!("JSON serialization failed: {}", vec_err));
            }
            match serde_json::from_str::<AgentAction>(&clean_text) {
                Ok(single) => Ok(vec![single]),
                Err(e) => Err(anyhow::anyhow!("JSON serialization failed: {}", e)),
            }
        }
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
            AgentAction::ReadFileChunk {
                _thought_process: None,
                path,
                start_line,
                max_lines,
                ..
            } => {
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
            AgentAction::ReadFileChunk {
                _thought_process: None,
                path,
                start_line,
                max_lines,
                ..
            } => {
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
            AgentAction::ReadFileChunk {
                _thought_process: None,
                path,
                start_line,
                max_lines,
                ..
            } => {
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
            AgentAction::ReadFileChunk {
                _thought_process: None, start_line, max_lines, ..
            } => {
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
            AgentAction::ReplaceLines { _thought_process: None, start_line, end_line, .. } => {
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
            AgentAction::WriteFile { _thought_process: None, path, content, .. } => {
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
            AgentAction::WriteFile { _thought_process: None, path, content, .. } => {
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
            AgentAction::ReadFileChunk {
                _thought_process: None,
                path,
                start_line,
                max_lines,
                ..
            } => {
                assert_eq!(path, "src/lib.rs");
                assert_eq!(start_line, 100);
                assert_eq!(max_lines, 50);
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }

    #[test]
    fn replace_lines_accepts_new_content_alias() {
        let action = extract_single(
            r#"[{
                "type": "replace_lines",
                "path": "bins/ide_backend/src/main.rs",
                "start_line": 228,
                "end_line": 228,
                "new_content": "            return;",
                "reasoning": "graceful shutdown"
            }]"#,
        );
        match action {
            AgentAction::ReplaceLines { path, start_line, end_line, replacement, .. } => {
                assert_eq!(path, "bins/ide_backend/src/main.rs");
                assert_eq!(start_line, 228);
                assert_eq!(end_line, 228);
                assert_eq!(replacement, "            return;");
            }
            other => panic!("expected ReplaceLines, got {:?}", other),
        }
    }

    #[test]
    fn replace_lines_still_accepts_native_replacement() {
        let action = extract_single(
            r#"[{
                "type": "replace_lines",
                "path": "src/lib.rs",
                "start_line": 10,
                "end_line": 12,
                "replacement": "fn foo() {}",
                "reasoning": "native form"
            }]"#,
        );
        match action {
            AgentAction::ReplaceLines { replacement, .. } => {
                assert_eq!(replacement, "fn foo() {}");
            }
            other => panic!("expected ReplaceLines, got {:?}", other),
        }
    }

    #[test]
    fn replace_lines_accepts_content_alias() {
        let action = extract_single(
            r#"[{
                "type": "replace_lines",
                "path": "a.rs",
                "start_line": 1,
                "end_line": 1,
                "content": "// replaced",
                "reasoning": ""
            }]"#,
        );
        match action {
            AgentAction::ReplaceLines { replacement, .. } => {
                assert_eq!(replacement, "// replaced");
            }
            other => panic!("expected ReplaceLines, got {:?}", other),
        }
    }
}

#[cfg(test)]
mod react_shape_tests {
    use super::extract_actions;
    use crate::agent_protocol::AgentAction;

    #[test]
    fn action_input_nesting_is_hoisted() {
        let raw = r#"{"action": "read_file_chunk", "action_input": {"filepath": "a.rs", "start_line": 10, "max_lines": 50}, "reasoning": "next chunk"}"#;
        let acts = extract_actions(raw).expect("parses");
        assert_eq!(acts.len(), 1);
        assert!(matches!(acts[0], AgentAction::ReadFileChunk { .. }), "{:?}", acts);
    }

    #[test]
    fn halt_object_shapes_are_normalized() {
        let flat = r#"{"halt": true, "reasoning": "ROOT_CAUSE: a. TRACE: b. FIX: c."}"#;
        let acts = extract_actions(flat).expect("parses");
        assert!(matches!(acts.first(), Some(AgentAction::Halt { .. })), "{:?}", acts);

        let nested = r#"{"halt": {"reasoning": {"ROOT_CAUSE": "a", "TRACE": "b", "FIX": "c"}}}"#;
        let acts = extract_actions(nested).expect("parses");
        assert!(matches!(acts.first(), Some(AgentAction::Halt { .. })), "{:?}", acts);
    }
}

#[cfg(test)]
mod conversational_recovery_tests {
    use super::extract_actions;
    use crate::agent_protocol::AgentAction;

    #[test]
    fn recovers_json_array_prefixed_with_conversational_text() {
        let payload = r#"We need to continue audit. Need read remaining lines. Here is the action: [{"type":"read_file_chunk","path":"crates/runtime_core/src/cortical_loop.rs","start_line":1644,"max_lines":250,"reasoning":"continue audit"}]"#;
        let actions = extract_actions(payload)
            .expect("should extract embedded JSON array from conversational text");
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => {
                assert!(path.ends_with("cortical_loop.rs"));
                assert_eq!(*start_line, 1644);
                assert_eq!(*max_lines, 250);
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }

    #[test]
    fn bare_object_with_brackets_in_reasoning_parses() {
        let payload = r#"{"type": "halt", "reasoning": "Verified `[READ_OK]` and `[HALT_REJECTED]` handling across `verifier.rs`."}"#;
        let actions = extract_actions(payload).expect("bare JSON object with brackets must parse");
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AgentAction::Halt { .. }), "{:?}", actions);
    }

    #[test]
    fn prefixed_object_with_brackets_in_reasoning_parses() {
        let payload = r#"Final action: {"type": "halt", "reasoning": "Checked `[CMD_OK]` and `[SEARCH_CODE_OK]` paths."}"#;
        let actions =
            extract_actions(payload).expect("prefixed JSON object with brackets must parse");
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AgentAction::Halt { .. }), "{:?}", actions);
    }

    #[test]
    fn concatenated_objects_take_first_value() {
        let payload = r#"{"type": "read_file", "path": "a.rs", "reasoning": "x"}{"type": "read_file", "path": "b.rs", "reasoning": "y"}"#;
        let actions = extract_actions(payload).expect("first of concatenated objects must parse");
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn newline_separated_arrays_take_first_array() {
        let payload = "[{\"type\": \"read_file\", \"path\": \"a.rs\", \"reasoning\": \"x\"}]\n[{\"type\": \"read_file\", \"path\": \"b.rs\", \"reasoning\": \"y\"}]";
        let actions = extract_actions(payload).expect("first of concatenated arrays must parse");
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn long_halt_with_markers_and_escaped_newlines_parses() {
        let body = "Q1 `[HALT_REJECTED]` and `[DAG NODE COMPLETE]`.\\n".repeat(80);
        let payload = format!(r#"[{{"type": "halt", "reasoning": "{}"}}]"#, body);
        let actions = extract_actions(&payload).expect("long bracketed halt must parse");
        assert!(matches!(actions[0], AgentAction::Halt { .. }));
    }
}

#[cfg(test)]
mod envelope_unwrap_tests {
    use super::extract_actions;
    use crate::agent_protocol::AgentAction;

    #[test]
    fn unwraps_reasoning_actions_envelope() {
        let raw = r#"{"reasoning": "attempt 1", "actions": [{"type":"write_file","path":"a.md","content":"hi","reasoning":"r"}]}"#;
        let parsed = extract_actions(raw).expect("plan-then-execute envelope must unwrap");
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            AgentAction::WriteFile { path, content, .. } => {
                assert_eq!(path, "a.md");
                assert_eq!(content, "hi");
            }
            other => panic!("expected WriteFile, got {:?}", other),
        }
    }

    #[test]
    fn unwraps_tool_args_envelope_and_aliases_filepath() {
        let raw = r#"{"tool": "read_file_chunk", "args": {"filepath": "src/x.rs", "start_line": 100, "max_lines": 200, "reasoning": "r"}}"#;
        let parsed = extract_actions(raw).expect("function-call envelope must unwrap");
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            AgentAction::ReadFileChunk { path, start_line, max_lines, .. } => {
                assert_eq!(path, "src/x.rs", "filepath must be aliased to path");
                assert_eq!(*start_line, 100);
                assert_eq!(*max_lines, 200);
            }
            other => panic!("expected ReadFileChunk, got {:?}", other),
        }
    }

    #[test]
    fn bare_action_array_still_parses() {
        let raw = r#"[{"type":"read_file","path":"a.rs","reasoning":"r"}]"#;
        let parsed = extract_actions(raw).expect("bare array must be unaffected");
        assert_eq!(parsed.len(), 1);
        assert!(matches!(parsed[0], AgentAction::ReadFile { .. }));
    }
}
