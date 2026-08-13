#![allow(warnings)]
use crate::json_parser::sanitize_json_escapes;
use crate::model_config::{get_heavy_models, get_light_models};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct CriticResponse {
    pub critique: String,
    pub confidence_modifier: f32,
}

pub struct EpistemicEngine;

impl EpistemicEngine {
    pub async fn safe_api_call(
        client: &Client,
        api_key: &str,
        models: &[String],
        payload: &Value,
    ) -> Result<String> {
        let mut last_err = anyhow!("No models available in cascade.");
        for model_endpoint in models {
            let is_nvidia = model_endpoint.starts_with("nvidia/");
            let short_name = model_endpoint.split('/').last().unwrap_or(model_endpoint);
            let mut retries = 0;

            loop {
                let (url, req_payload, use_auth) = if is_nvidia {
                    let nv_key = std::env::var("NVIDIA_API_KEY")
                        .unwrap_or_default()
                        .trim()
                        .trim_matches('"')
                        .to_string();
                    let text = payload
                        .pointer("/contents/0/parts/0/text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let mut p = serde_json::json!({
                        "model": model_endpoint,
                        "messages": [{"role": "user", "content": text}],
                        "max_tokens": 4096,
                        "temperature": 0.1
                    });

                    // ⚡ V6 NATIVE TOOL CALLING: Force JSON Schema for Nvidia/OpenAI
                    if let Some(mime) = payload
                        .pointer("/generationConfig/responseMimeType")
                        .and_then(|v| v.as_str())
                    {
                        if mime == "application/json" {
                            p.as_object_mut().unwrap().insert(
                                "response_format".to_string(),
                                serde_json::json!({"type": "json_object"}),
                            );
                            let msg = format!(
                                "{}\n\nCRITICAL: You must return a valid JSON object.",
                                text
                            );
                            p["messages"][0]["content"] = serde_json::json!(msg);
                        }
                    }

                    (
                        "https://generativelanguage.googleapis.com/v1beta/chat/completions".to_string(),
                        p,
                        Some(nv_key),
                    )
                } else {
                    let key = if api_key.is_empty() {
                        std::env::var("GEMINI_API_KEY").unwrap_or_default().trim().to_string()
                    } else {
                        api_key.to_string()
                    };
                    let u = format!(
                        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                        model_endpoint.replace("models/", ""),
                        key
                    );
                    (u, payload.clone(), None)
                };

                let mut req = client.post(&url).json(&req_payload);
                if let Some(auth_key) = use_auth {
                    req = req.header("Authorization", format!("Bearer {}", auth_key));
                }

                let res = req.send().await?;
                let body: Value = match res.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        last_err = anyhow::anyhow!("JSON Parse Error: {}", e);
                        retries += 1;
                        if retries > 2 {
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        continue;
                    }
                };

                if is_nvidia {
                    if body.get("error").is_some() {
                        last_err = anyhow!("NVIDIA API Error: {:?}", body);
                        break;
                    }
                } else {
                    if let Some(err) = body.pointer("/error/message").and_then(|v| v.as_str()) {
                        last_err = anyhow!("Gemini API Error: {}", err);
                        if err.contains("quota")
                            || err.contains("rate limit")
                            || err.contains("RESOURCE_EXHAUSTED")
                        {
                            crate::model_config::report_rate_limit(model_endpoint);
                            break;
                        }
                        if err.contains("Internal error")
                            || err.contains("quota")
                            || err.contains("rate limit")
                            || err.contains("503")
                        {
                            retries += 1;
                            if retries > 6 {
                                break;
                            }
                            let backoff = std::cmp::min(2_u64.pow(retries as u32) * 2, 45);
                            tokio::time::sleep(Duration::from_secs(backoff)).await;
                            continue;
                        }
                        break;
                    }
                }

                let mut final_text = String::new();
                if is_nvidia {
                    if let Some(content) =
                        body.pointer("/choices/0/message/content").and_then(|v| v.as_str())
                    {
                        final_text = content.to_string();
                    }
                    if final_text.trim().is_empty() {
                        if let Some(rc) = body
                            .pointer("/choices/0/message/reasoning_content")
                            .and_then(|v| v.as_str())
                        {
                            final_text = rc.to_string();
                        }
                    }
                } else {
                    if let Some(parts) =
                        body.pointer("/candidates/0/content/parts").and_then(|v| v.as_array())
                    {
                        for part in parts {
                            if part.get("thought").and_then(|t| t.as_bool()).unwrap_or(false) {
                                continue;
                            }
                            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                final_text.push_str(text);
                            }
                        }
                    }
                }

                if final_text.trim().is_empty() {
                    retries += 1;
                    if retries > 4 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(2_u64.pow(retries as u32) * 2)).await;
                    continue;
                }
                return Ok(final_text.replace("```json", "").replace("```", "").trim().to_string());
            }
        }
        Err(anyhow!("Model Cascade Exhausted. Final error: {}", last_err))
    }

    pub async fn generate_hypotheses(
        client: &Client,
        api_key: &str,
        problem: &str,
        world_context: &str,
    ) -> Result<Vec<String>> {
        let prompt = format!("You are AthenaFusionX. Analyze this directive.\nDIRECTIVE: {}\nWORLD CONTEXT: {}\nCRITICAL RULE: If the directive explicitly tells you to execute a specific command, at least ONE of your strategies MUST be to simply execute that exact command.\nGenerate 3 distinct, actionable strategies. OUTPUT ONLY A JSON ARRAY OF STRINGS. No markdown.", problem, world_context);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 2048 }});
        let text = Self::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_heavy_models(),
            &payload,
        )
        .await?;
        let clean_text = sanitize_json_escapes(&text);

        if let Ok(hyps) = serde_json::from_str::<Vec<String>>(&clean_text) {
            if !hyps.is_empty() {
                return Ok(hyps);
            }
        }

        let lines: Vec<String> = text
            .lines()
            .map(|l| {
                l.trim()
                    .trim_start_matches(|c: char| {
                        c == '-'
                            || c == '*'
                            || c == '•'
                            || c.is_ascii_digit()
                            || c == '.'
                            || c == '>'
                    })
                    .trim()
                    .to_string()
            })
            .filter(|l| !l.is_empty() && l.len() > 5)
            .take(3)
            .collect();

        if !lines.is_empty() {
            Ok(lines)
        } else {
            Ok(vec![format!("Execute directive: {}", problem)])
        }
    }

    pub async fn propose_action(
    client: &Client,
    api_key: &str,
    mission: &str,
    strategy: &str,
    world_context: &str,
    evidence_so_far: &str,
) -> Result<String> {
    let prompt = format!(
        "You are the Prefrontal and Synthesizer Cortex. Your role is purely strategic, creative, and intellectual.
\
        ORIGINAL DIRECTIVE: {}
\
        STRATEGY: {}
\
        WORLD CONTEXT: {}
\
        EVIDENCE GATHERED SO FAR:
{}
\
        Propose the exact next step to fulfill the strategy.
\
        CRITICAL RULE 0: NATIVE TOOLS ONLY (read_file, read_file_chunk, execute_command, patch_function, write_file, replace_lines, diff_patch, search_code, search_memory, run_tests, halt).
\
        CRITICAL RULE 1: If the EVIDENCE contains an exact file path outputted by a previous command, you MUST use that exact path.
\
        CRITICAL RULE 2: If EVIDENCE satisfies the strategy, declare HALT.
\
        CRITICAL RULE 3: For simple commands, state your intent in natural language. HOWEVER, if your action is write_file, replace_lines, or diff_patch, you MAY output the exact JSON array directly to prevent text corruption during downstream translation.
\
        CRITICAL RULE 4: If your decision involves writing a document, modifying code, or patching a file, you MUST generate the raw document text or exact source code entirely within your textual response.
\
        Think step by step. State the intended tool, the exact target path, and provide any required text/code cleanly. A downstream Motor Cortex will translate this into executable JSON formatting.",
        mission, strategy, world_context, evidence_so_far
    );

    let payload = serde_json::json!({
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
        "generationConfig": {
            "temperature": 0.1,
            "maxOutputTokens": 8192
        }
    });
    
    // Delegate to the Planner / Heavy loop to act as the primary intellectual engine
    Self::safe_api_call(client, api_key, &crate::model_config::get_heavy_models(), &payload).await
}

    pub async fn criticize_proposal(
        client: &Client,
        api_key: &str,
        mission: &str,
        strategy: &str,
        proposed_action: &str,
        world_context: &str,
        evidence_history: &str,
    ) -> Result<CriticResponse> {
        let prompt = format!("You are the Adversarial Critic.\nORIGINAL DIRECTIVE (paths, files, or resources explicitly named in this directive are PRE-AUTHORIZED and considered resolved -- do NOT veto access to them as path-guessing or as absent from the symbol_graph): {}\nSTRATEGY: {}\nPROPOSED ACTION: {}\nWORLD CONTEXT: {}\nEVIDENCE HISTORY (real observations already gathered this mission): {}\nEvaluate if this action is safe and logically sound. \nCRITICAL RULE 1 [AST-FIRST]: You must VETO any 'read_file' if the path is NOT explicitly named in the ORIGINAL DIRECTIVE AND is NOT present in the EVIDENCE HISTORY as an output from a previous tool call (such as search_code, find, or ls). Acting on a path that was outputted by a previous search/ls command in EVIDENCE HISTORY is VERIFIED and strictly AUTHORIZED—do NOT veto it as path guessing.\nCRITICAL RULE 2 [STRICT COMPLETION]: If the proposed action is to HALT or declare the task complete, you MUST VETO IT unless the EVIDENCE HISTORY definitively proves that ALL components of the current STRATEGY have been successfully executed and logged. A single success marker (like [CMD_OK] for a git clone) is NOT sufficient if the strategy requires multi-step synthesis (e.g., reading files AND generating a comparison document). If the strategy dictates creating or writing a document (e.g., a roadmap), you must see a [WRITE_OK] or [CMD_OK] specifically confirming that final file was written before approving a halt. Treat any unverified or partial completion claim as a reason for a negative confidence_modifier and mandate the missing execution step. CRITICAL RULE 4: CONSTRUCTIVE VETO. If you issue a negative confidence_modifier, your critique MUST explicitly state the exact command or action the Strategist should take next to escape the failure.\nCRITICAL RULE 3: THE EVIDENCE VERIFICATION TAXONOMY. If the proposed action contains a quantitative claim, architectural assumption, or performance metric (e.g., \"faster\", \"optimized\", \"40x\"), you MUST check the EVIDENCE HISTORY. If there is no explicit [Measured] execution log proving the claim, you MUST VETO the proposal (provide a negative confidence_modifier) and cite: \"VETO: Unverified quantitative claim. Downgrade to [Hypothesis] or provide [Measured] evidence.\"\nOUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", mission, strategy, proposed_action, world_context, evidence_history);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024 }});
        let text = Self::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_light_models(),
            &payload,
        )
        .await?;
        let clean_text = sanitize_json_escapes(&text);
        serde_json::from_str(&clean_text).map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub async fn translate_to_action(
    client: &reqwest::Client,
    api_key: &str,
    _strategy: &str,
    proposed_action: &str,
) -> Result<String> {
    let prompt = format!(
        "Translate the SYSTEM INTENT into the corresponding JSON ARRAY block.\n\
        \n\
        [EXAMPLE]\n\
        SYSTEM INTENT: I need to use execute_command to run git clone.\n\
`````json\n\
        [{{\"type\": \"execute_command\", \"command\": \"git clone\", \"reasoning\": \"Downloading code\"}}]\n\
````\n\
        \n\
        [ALLOWED ACTIONS]\n\
        - execute_command (needs: 'command')\n\
        - read_file (needs: 'path')\n\
        - read_file_chunk (needs: 'path', 'start_line', 'max_lines')\n\
        - write_file (needs: 'path', 'content')\n\
        - replace_lines (needs: 'path', 'start_line', 'end_line', 'replacement')\n\
        - diff_patch (needs: 'path', 'search', 'replace')\n\
        - patch_function (needs: 'path', 'function_name', 'new_body')\n\
        - search_code (needs: 'path', 'query')\n\
        - search_memory (needs: 'query')\n\
        - run_tests (needs: 'test_target')\n\
        - halt (needs: 'reasoning')\n\
        \n\
        [ACTUAL TASK]\n\
        SYSTEM INTENT: {}\n\
```json\n",
        proposed_action
    );

    let payload = serde_json::json!({
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
        "generationConfig": {
            "temperature": 0.0,
            "maxOutputTokens": 2048
        }
    });

    Self::safe_api_call(client, api_key, &crate::model_config::get_light_models(), &payload).await
}

    pub async fn evaluate_evidence(
        client: &Client,
        api_key: &str,
        strategy: &str,
        action: &str,
        evidence: &str,
    ) -> Result<CriticResponse> {
        let truncated_evidence: String = evidence.chars().take(8000).collect();
        let prompt = format!("You are the Scientific Evaluator.\nSTRATEGY: {}\nACTION TAKEN: {}\nEVIDENCE GATHERED:\n{}\nDoes this evidence represent meaningful partial progress toward the strategy? If it gathers useful context (like listing a directory or reading a file), give a positive modifier. DO NOT demand the entire strategy be completed in one step.\nNEGATIVE SPACE RULE: If the strategy is to search for a contract, pattern, or file, and the ACTION TAKEN (like search_code or find) definitively proves the object DOES NOT EXIST (e.g., 'No code matches found'), this is NOT a failure. Proving non-existence is a SUCCESSFUL resolution of the search. Give a POSITIVE modifier, NOT negative.\nHALT RULE: If ACTION TAKEN is 'halt' and the EVIDENCE GATHERED (this turn or earlier in context) already shows a concrete success marker ([CMD_OK], [READ_OK], [WRITE_OK], [REPLACE_LINES_OK]) satisfying the strategy, this is a CORRECT completion, not a loss of progress -- give a positive modifier, not negative.\nMUTATION RULE: A successful mutating command (write, patch, mv, rm, mkdir, etc.) with [CMD_OK] and no stdout is genuine positive progress, not neutral reconnaissance -- do not score it 0.0 solely for lacking output.\nOUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", strategy, action, truncated_evidence);
        let payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "generationConfig": {
                "temperature": 0.0,
                "responseMimeType": "application/json",
                "maxOutputTokens": 4096,
                "responseSchema": {
                    "type": "OBJECT",
                    "properties": {
                        "critique": { "type": "STRING" },
                        "confidence_modifier": { "type": "NUMBER" }
                    },
                    "required": ["critique", "confidence_modifier"]
                }
            }
        });
        let text = Self::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_light_models(),
            &payload,
        )
        .await?;
        let clean_text = sanitize_json_escapes(&text);
        serde_json::from_str(&clean_text).map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub async fn verify_outcome(
        client: &Client,
        api_key: &str,
        strategy: &str,
        action: &str,
        evidence: &str,
    ) -> Result<crate::cognition_graph::VerificationReport> {
        let truncated_evidence: String = evidence.chars().take(8000).collect();
        let truncated_action: String = action.chars().take(2000).collect();
        let prompt = format!(
            "You are the Epistemic Verifier.\nSTRATEGY: {}\nACTION TAKEN: {}\nEVIDENCE:\n{}\n\
            Does this evidence mathematically prove the stated strategy's objective was achieved? \
            A successful action (like [WRITE_OK] or [CMD_OK]) is NOT proof of success on its own UNLESS the ACTION TAKEN itself contains the actual content that was written or patched (write_file/diff_patch/patch_function actions above include the real file content directly in ACTION TAKEN) — in that case, check whether that content satisfies the objective, and treat a confirmed match as proof. Do not reject a successful write or patch merely because the bare EVIDENCE line (e.g. [WRITE_OK]) doesn't repeat the content; the content to check is in ACTION TAKEN. \
            OUTPUT ONLY VALID JSON matching this schema:\n\
            {{\"goal_satisfied\": true, \"evidence_confidence\": 0.9, \"independent_check_passed\": true, \"rationale\": \"<reason>\"}}",
            strategy, truncated_action, truncated_evidence
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.1, "maxOutputTokens": 1024 }});
        let text = Self::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_heavy_models(),
            &payload,
        )
        .await?;
        let clean_text = sanitize_json_escapes(&text);
        serde_json::from_str(&clean_text).map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub async fn reflect_on_failure(
        client: &Client,
        api_key: &str,
        strategy: &str,
        evidence: &str,
    ) -> Result<String> {
        let truncated_evidence: String = evidence.chars().take(8000).collect();
        let prompt = format!(
            "You are the Meta-Cognitive Reflector.\nSTRATEGY: {}\nEVIDENCE (Failure/Anomaly):\n{}\n\
            Analyze why the strategy failed to achieve the objective based on the evidence. \
            Provide a concise 2-sentence reflection on the error and suggest a pivot. OUTPUT ONLY PLAIN TEXT.",
            strategy, truncated_evidence
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.3, "maxOutputTokens": 1024 }});
        Self::safe_api_call(client, api_key, &crate::model_config::get_light_models(), &payload)
            .await
    }
}
