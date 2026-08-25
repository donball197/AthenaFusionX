import sys

PATH = "crates/runtime_core/src/executor.rs"

ANCHOR = '''            MotorAction::SynthesizeReport { output_path, source_files } => {
                let mut combined_context = String::new();
                for file in source_files {
                    if let Ok(safe_p) = sanitize(&file) {
                        if let Ok(content) = tokio::fs::read_to_string(&safe_p).await {
                            combined_context.push_str(&format!("\\n--- FILE: {} ---\\n{}\\n", file, content));
                        }
                    }
                }
                
                let prompt = format!("You are a Synthesis Subagent. Analyze the following source code files and output a comprehensive architectural markdown report. DO NOT output JSON. Output raw markdown only.\\n\\nFILES:\\n{}", combined_context);
                
                let payload = serde_json::json!({
                    "contents": [{"role": "user", "parts": [{"text": prompt}]}],
                    "generationConfig": { "temperature": 0.2, "maxOutputTokens": 8192 }
                });
                
                let heavy_models = crate::model_config::get_heavy_models();
                let client = reqwest::Client::new();
                
                match crate::epistemic_engine::EpistemicEngine::safe_api_call(&client, &self.api_key, &heavy_models, &payload).await {
                    Ok(markdown) => {
                        let raw_path = Path::new(&output_path);
                        if raw_path.is_absolute() { return Err(anyhow::anyhow!("PATH_TRAVERSAL_DENIED")); }
                        let ws = std::env::current_dir().unwrap().canonicalize().unwrap();
                        let safe_path = ws.join(raw_path);
                        if !safe_path.starts_with(&ws) { return Err(anyhow::anyhow!("PATH_TRAVERSAL_DENIED")); }
                        
                        if let Some(parent) = safe_path.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        tokio::fs::write(&safe_path, markdown).await?;
                        Ok(format!("[WRITE_OK] Successfully synthesized and wrote report to {}", output_path))
                    }
                    Err(e) => Err(anyhow::anyhow!("Synthesis Failed: {}", e))
                }
            }'''

REPLACEMENT = '''            MotorAction::SynthesizeReport { output_path, source_files } => {
                let mut combined_context = String::new();
                let mut read_log: Vec<String> = Vec::new();
                let mut read_ok_count = 0usize;
                for file in &source_files {
                    match sanitize(file) {
                        Ok(safe_p) => match tokio::fs::read_to_string(&safe_p).await {
                            Ok(content) => {
                                read_log.push(format!("[READ_OK] {} ({} bytes)", file, content.len()));
                                read_ok_count += 1;
                                combined_context.push_str(&format!("\\n--- FILE: {} ---\\n{}\\n", file, content));
                            }
                            Err(e) => {
                                read_log.push(format!("[READ_FAIL] {}: {}", file, e));
                            }
                        },
                        Err(e) => {
                            read_log.push(format!("[READ_FAIL] {}: path rejected by sanitize ({})", file, e));
                        }
                    }
                }

                if read_ok_count == 0 {
                    return Err(anyhow::anyhow!(
                        "SYNTHESIZE_REPORT_NO_SOURCES: 0 of {} listed source files were readable:\\n{}",
                        source_files.len(),
                        read_log.join("\\n")
                    ));
                }

                let prompt = format!("You are a Synthesis Subagent. Analyze the following source code files and output a comprehensive architectural markdown report. DO NOT output JSON. Output raw markdown only.\\n\\nFILES:\\n{}", combined_context);
                
                let payload = serde_json::json!({
                    "contents": [{"role": "user", "parts": [{"text": prompt}]}],
                    "generationConfig": { "temperature": 0.2, "maxOutputTokens": 8192 }
                });
                
                let heavy_models = crate::model_config::get_heavy_models();
                let client = reqwest::Client::new();
                
                match crate::epistemic_engine::EpistemicEngine::safe_api_call(&client, &self.api_key, &heavy_models, &payload).await {
                    Ok(markdown) => {
                        let raw_path = Path::new(&output_path);
                        if raw_path.is_absolute() { return Err(anyhow::anyhow!("PATH_TRAVERSAL_DENIED")); }
                        let ws = std::env::current_dir().unwrap().canonicalize().unwrap();
                        let safe_path = ws.join(raw_path);
                        if !safe_path.starts_with(&ws) { return Err(anyhow::anyhow!("PATH_TRAVERSAL_DENIED")); }
                        
                        if let Some(parent) = safe_path.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        tokio::fs::write(&safe_path, markdown).await?;
                        Ok(format!(
                            "{}\\n[WRITE_OK] Successfully synthesized and wrote report to {} ({}/{} source files read)",
                            read_log.join("\\n"),
                            output_path,
                            read_ok_count,
                            source_files.len()
                        ))
                    }
                    Err(e) => Err(anyhow::anyhow!("Synthesis Failed: {}", e))
                }
            }'''

with open(PATH, "r") as f:
    content = f.read()

count = content.count(ANCHOR)
if count != 1:
    print(f"FATAL: anchor matched {count} times (expected exactly 1). Aborting.")
    sys.exit(1)

content = content.replace(ANCHOR, REPLACEMENT)

with open(PATH, "w") as f:
    f.write(content)

print("OK: patched SynthesizeReport to log per-file read evidence and fail on zero readable sources.")
