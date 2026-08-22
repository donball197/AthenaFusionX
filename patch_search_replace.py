# --- CLIENT.RS PATCH ---
try:
    with open('apps/nexus_ast_tool/src/client.rs', 'r') as f:
        client_code = f.read()

    start_client = client_code.find("pub fn generate_security_patch(")
    end_client = client_code.find("fn resolve_model()")

    if start_client == -1 or end_client == -1:
        print("ERROR: Could not find bounds in client.rs")
    else:
        new_client_funcs = """pub fn generate_security_patch(
        finding_type: &str,
        code_snippet: &str,
        lang: &str,
    ) -> Result<(String, String)> {
        Self::with_retry(
            || Self::generate_security_patch_attempt(finding_type, code_snippet, lang),
            3,
        )
    }

    fn generate_security_patch_attempt(
        finding_type: &str,
        code_snippet: &str,
        lang: &str,
    ) -> Result<(String, String)> {
        std::thread::sleep(std::time::Duration::from_millis(2000));

        let model = Self::resolve_model()?;
        let model_lower = model.to_lowercase();
        let is_gemini = model_lower.contains("gemini") || model_lower.contains("gemma");

        let api_key = if is_gemini {
            std::env::var("GEMINI_API_KEY").map_err(|_| anyhow::anyhow!("GEMINI_API_KEY required"))?
        } else {
            std::env::var("NVIDIA_API_KEY").map_err(|_| anyhow::anyhow!("NVIDIA_API_KEY required"))?
        };

        let endpoint = if is_gemini {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model, api_key
            )
        } else {
            "https://integrate.api.nvidia.com/v1/chat/completions".to_string()
        };

        let prompt = format!(
            "You are an elite Security Auto-Healer AI.\\n\\
             Fix this '{}' vulnerability in this {} code:\\n\\
             ```\\n\\
             {}\\n\\
             ```\\n\\
             CRITICAL: You must use a SEARCH/REPLACE block. Identify the EXACT vulnerable lines to remove (including exact indentation) and provide the secure replacement.\\n\\
             Output EXACTLY in this format:\\n\\
             <<<< SEARCH\\n\\
             [exact code to remove]\\n\\
             ====\\n\\
             [secure replacement code]\\n\\
             >>>> REPLACE\\n\\
             Do NOT wrap the output in markdown fences or XML tags. Ensure the SEARCH block matches the file exactly.",
            finding_type, lang, code_snippet
        );

        let body = if is_gemini {
            serde_json::json!({ "contents": [{ "parts": [{ "text": prompt }] }] })
        } else {
            serde_json::json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "temperature": 0.1
            })
        };

        let mut req = ureq::post(&endpoint).set("Content-Type", "application/json");
        if !is_gemini {
            req = req.set("Authorization", &format!("Bearer {}", api_key));
        }

        let response = req.send_json(body).map_err(|e| anyhow::anyhow!("HTTP req failed: {e}"))?;
        let text = Self::extract_text(response, is_gemini)?;

        let search_start = text.find("<<<< SEARCH").ok_or_else(|| anyhow::anyhow!("Missing <<<< SEARCH in payload"))?;
        let delim = text.find("====").ok_or_else(|| anyhow::anyhow!("Missing ==== in payload"))?;
        let replace_end = text.find(">>>> REPLACE").ok_or_else(|| anyhow::anyhow!("Missing >>>> REPLACE in payload"))?;

        let search_block = text[search_start + 11..delim].trim_matches('\\n').to_string();
        let replace_block = text[delim + 4..replace_end].trim_matches('\\n').to_string();

        Ok((search_block, replace_block))
    }

"""
        with open('apps/nexus_ast_tool/src/client.rs', 'w') as f:
            f.write(client_code[:start_client] + new_client_funcs + client_code[end_client:])
        print("✅ client.rs patched successfully.")
except Exception as e:
    print(f"Error patching client.rs: {e}")

# --- MAIN.RS PATCH ---
try:
    with open('apps/nexus_ast_tool/src/main.rs', 'r') as f:
        main_code = f.read()

    start_main = main_code.find("if auto_heal {")
    end_main = main_code.find("report_md.push_str(\"---\\n\\n\");", start_main)

    if start_main == -1 or end_main == -1:
        print("ERROR: Could not find bounds in main.rs")
    else:
        new_main_block = """if auto_heal {
                                println!("        [🛠️ Auto-Heal] Generating secure patch...");
                                match nexus_ast_tool::client::ApiClient::generate_security_patch(&finding.pattern_name, &finding.source_snippet, &lang_str) {
                                    Ok((search_block, replace_block)) => {
                                        if let Ok(file_source) = std::fs::read_to_string(&finding.path) {
                                            if !file_source.contains(&search_block) {
                                                println!("        [❌ Patch Rejected] SEARCH block not found exactly in file. Reverting.");
                                                report_md.push_str(&format!(
                                                    "### Recommended Patch (auto-heal attempted, REJECTED: SEARCH block mismatch, reverted)\\n```{} \\n{}\\n```\\n\\n",
                                                    lang_str.to_lowercase(), replace_block
                                                ));
                                            } else {
                                                let new_source = file_source.replace(&search_block, &replace_block);
                                                let _ = std::fs::write(&finding.path, &new_source);

                                                let syntax_ok = verify_syntax(&lang_str, &finding.path);
                                                let mut ast_ok = false;

                                                if syntax_ok {
                                                    let post_findings = PolyglotScanner::scan_security_vulnerabilities(&finding.path, &rules);
                                                    ast_ok = !post_findings.iter().any(|f| f.pattern_name == finding.pattern_name);
                                                }

                                                if syntax_ok && (ast_ok || contains_safe_pattern(&finding.pattern_name, &replace_block)) {
                                                    println!("        [✅ Patch Verified] Syntax OK and [Safe Pattern / AST Rescan] confirms vulnerability eliminated.");
                                                    report_md.push_str("### Recommended Patch (Auto-Healed)\\n");
                                                    report_md.push_str(&format!("```{} \\n{}\\n```\\n\\n", lang_str.to_lowercase(), replace_block));
                                                } else {
                                                    let reject_reason = if !syntax_ok { "Syntax check failed" } else { "AST Rescan detected vulnerability still present" };
                                                    println!("        [❌ Patch Rejected] {}. Reverting.", reject_reason);
                                                    let _ = std::fs::write(&finding.path, &file_source);
                                                    report_md.push_str(&format!(
                                                        "### Recommended Patch (auto-heal attempted, REJECTED: {}, reverted)\\n```{} \\n{}\\n```\\n\\n",
                                                        reject_reason, lang_str.to_lowercase(), replace_block
                                                    ));
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => println!("        [❌ Patch Gen Error] {}", e),
                                }
                            }
                            """
        with open('apps/nexus_ast_tool/src/main.rs', 'w') as f:
            f.write(main_code[:start_main] + new_main_block + main_code[end_main:])
        print("✅ main.rs patched successfully.")
except Exception as e:
    print(f"Error patching main.rs: {e}")
