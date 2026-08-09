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
                    if let Some(mime) = payload.pointer("/generationConfig/responseMimeType").and_then(|v| v.as_str()) {
                        if mime == "application/json" {
                            p.as_object_mut().unwrap().insert("response_format".to_string(), serde_json::json!({"type": "json_object"}));
                            let msg = format!("{}\n\nCRITICAL: You must return a valid JSON object.", text);
                            p["messages"][0]["content"] = serde_json::json!(msg);
                        }
                    }

                    (
                        "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
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
                        if retries > 2 { break; }
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
                        if err.contains("quota") || err.contains("rate limit") || err.contains("RESOURCE_EXHAUSTED") {
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
                        if let Some(rc) =
                            body.pointer("/choices/0/message/reasoning_content").and_then(|v| v.as_str())
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
            .map(|l| l.trim().trim_start_matches(|c: char| c == '-' || c == '*' || c == '•' || c.is_ascii_digit() || c == '.' || c == '>').trim().to_string())
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
        let prompt = format!("You are the Action Strategist.
ORIGINAL DIRECTIVE: {}
STRATEGY: {}
WORLD CONTEXT: {}
EVIDENCE GATHERED SO FAR:
{}
Propose the exact next step to fulfill the strategy.
CRITICAL RULE 0: Do NOT propose directory-listing or file-discovery commands (ls -R, find, cat on an unconfirmed path) if the relevant file tree or path is already visible in WORLD CONTEXT above.
CRITICAL RULE 1: If the EVIDENCE GATHERED SO FAR contains an exact file path outputted by symbol_graph or a previous command, you MUST use that exact path.
CRITICAL RULE 2: If the EVIDENCE GATHERED SO FAR contains a Critic rejection or veto (marked [Contradicts]) that suggests a specific tool, command, or action to try next, your proposal MUST adopt that suggested action. DO NOT RE-PROPOSE AN ACTION THAT WAS JUST REJECTED.
CRITICAL RULE 3: If the task requires generating a report, analysis, or diagram, you MUST propose writing the final synthesized output to a file using 'execute_command echo -e \"<content>\" > <path>' BEFORE you halt. If the EVIDENCE GATHERED SO FAR proves your current strategy is fully complete with a [WRITE_OK] or [CMD_OK] output satisfying the directive, your IMMEDIATE next proposal MUST be 'halt'.
CRITICAL RULE 4: TOOLCHAIN TRUST & EXPLORATORY GRACE
If the Strategist proposes executing a workspace tool (e.g., benchmark_runner, mutation_ranker, execution_auditor, impact_analyzer) via target/release/ or cargo run, you MUST ASSUME the tool exists and is authorized. Do NOT veto execution on the basis of missing prerequisite files (like trace logs, json reports, or candidates) if the tool is actively being invoked to analyze a target. Allow exploratory tool execution. Do not repeat successful commands.
CRITICAL RULE 5: When invoking workspace binaries (impact_analyzer, execution_auditor, mutation_ranker, symbol_graph), ALWAYS use 'cargo run --bin <bin_name> -- <args>' or the absolute debug path '/home/donball197/AthenaFusionX/target/debug/<bin_name>'. NEVER execute 'apps/<bin_name>' directly because 'apps/' is a source directory, not an executable binary.
CRITICAL RULE 6: SCRIPT ENVIRONMENT: Any proposed inline scripts (Python/Bash) MUST rely strictly on standard system libraries (e.g., os, sys, json in Python). Do NOT assume third-party packages like 'toml' or 'requests' are installed.
OUTPUT ONLY A SINGLE LINE CONTAINING THE EXACT COMMAND OR ACTION. DO NOT INCLUDE EXPLANATIONS, REASONING, OR PREAMBLE.", mission, strategy, world_context, evidence_so_far);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.2, "maxOutputTokens": 1024 }});
        Self::safe_api_call(client, api_key, &crate::model_config::get_light_models(), &payload)
            .await
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
        let prompt = format!("You are the Adversarial Critic.\nORIGINAL DIRECTIVE (paths, files, or resources explicitly named in this directive are PRE-AUTHORIZED and considered resolved -- do NOT veto access to them as path-guessing or as absent from the symbol_graph): {}\nSTRATEGY: {}\nPROPOSED ACTION: {}\nWORLD CONTEXT: {}\nEVIDENCE HISTORY (real observations already gathered this mission): {}\nEvaluate if this action is safe and logically sound. \nCRITICAL RULE 1 [AST-FIRST]: You must VETO any 'read_file' or OS shell search ('find', 'grep') if the absolute workspace path has not been explicitly resolved via the symbol_graph AND is not explicitly named in the ORIGINAL DIRECTIVE above. Path guessing is STRICTLY PROHIBITED, but acting on a path the directive itself already named is not guessing.\nCRITICAL RULE 2: If the proposed action is to HALT or declare the task complete, you MUST APPROVE IT ONLY IF the EVIDENCE HISTORY above already contains a concrete success marker ([WRITE_OK], [REPLACE_LINES_OK], [DIFF_PATCH_OK], [PATCH_FUNCTION_OK], or [CMD_OK] showing the specific outcome) that corroborates the exact claim being made. If the claimed completion is not backed by a matching entry in EVIDENCE HISTORY, it is an unverified assertion -- evaluate it under normal safety/soundness scrutiny instead of auto-approving, and treat an unsupported completion claim as a reason for a negative confidence_modifier. CRITICAL RULE 4: CONSTRUCTIVE VETO. If you issue a negative confidence_modifier, your critique MUST explicitly state the exact command or action the Strategist should take next to escape the failure.\nCRITICAL RULE 3: THE EVIDENCE VERIFICATION TAXONOMY. If the proposed action contains a quantitative claim, architectural assumption, or performance metric (e.g., \"faster\", \"optimized\", \"40x\"), you MUST check the EVIDENCE HISTORY. If there is no explicit [Measured] execution log proving the claim, you MUST VETO the proposal (provide a negative confidence_modifier) and cite: \"VETO: Unverified quantitative claim. Downgrade to [Hypothesis] or provide [Measured] evidence.\"\nOUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", mission, strategy, proposed_action, world_context, evidence_history);
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
        client: &Client,
        api_key: &str,
        strategy: &str,
        proposed_action: &str,
    ) -> Result<String> {
        let prompt = format!(
            "You are the Action Translator.\nSTRATEGY: {}\nPROPOSED ACTION: {}\nCRITICAL RULE: Translate the PROPOSED ACTION into exactly ONE JSON Array using the explicit JSON object schema below:\n\
             VALID JSON ACTIONS (MUST MATCH RUST ENUM EXACTLY):\n\
             1. [{{ \"type\": \"execute_command\", \"command\": \"<bash_command>\", \"reasoning\": \"...\" }}]\n\
             2. [{{ \"type\": \"search_memory\", \"query\": \"<search_query>\", \"reasoning\": \"...\" }}]\n\
             3. [{{ \"type\": \"read_file\", \"path\": \"<filepath>\", \"reasoning\": \"...\" }}]\n\
             4. [{{ \"type\": \"read_file_chunk\", \"path\": \"<filepath>\", \"start_line\": 0, \"max_lines\": 100, \"reasoning\": \"...\" }}]\n\
             5. [{{ \"type\": \"search_code\", \"path\": \"<filepath_or_dir>\", \"query\": \"<search_pattern>\", \"reasoning\": \"...\" }}]\n\
             6. [{{ \"type\": \"write_file\", \"path\": \"<filepath>\", \"content\": \"<EXACT_DATA_ESCAPED>\", \"reasoning\": \"...\" }}]\n\
             7. [{{ \"type\": \"replace_lines\", \"path\": \"<filepath>\", \"start_line\": 1, \"end_line\": 5, \"replacement\": \"<new_code>\", \"reasoning\": \"...\" }}]\n\
             8. [{{ \"type\": \"diff_patch\", \"path\": \"<filepath>\", \"search\": \"<search>\", \"replace\": \"<replace>\", \"reasoning\": \"...\" }}]\n\
             9. [{{ \"type\": \"patch_function\", \"path\": \"<filepath>\", \"function_name\": \"<name>\", \"new_body\": \"<full source>\", \"reasoning\": \"...\" }}]\n\
             10. [{{ \"type\": \"halt\", \"reasoning\": \"...\" }}]\n\
             NOTE: If the PROPOSED ACTION contains source code, you MUST escape all newlines (\\n) and double quotes (\\\") inside the 'content', 'replacement', or 'new_body' fields so the JSON parses correctly.\n\
             Select ONLY the relevant action object and enclose it in a JSON array. Do not output anything else.",
            strategy, proposed_action
        );
        let payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "generationConfig": {
                "temperature": 0.0,
                "responseMimeType": "application/json",
                "maxOutputTokens": 4096,
                "responseSchema": {
                    "type": "ARRAY",
                    "items": {
                        "type": "OBJECT",
                        "properties": {
                            "type": { "type": "STRING" },
                            "command": { "type": "STRING" },
                            "path": { "type": "STRING" },
                            "content": { "type": "STRING" },
                            "search": { "type": "STRING" },
                            "replace": { "type": "STRING" },
                            "function_name": { "type": "STRING" },
                            "new_body": { "type": "STRING" },
                            "replacement": { "type": "STRING" },
                            "query": { "type": "STRING" },
                            "start_line": { "type": "INTEGER" },
                            "end_line": { "type": "INTEGER" },
                            "max_lines": { "type": "INTEGER" },
                            "reasoning": { "type": "STRING" }
                        },
                        "required": ["type", "reasoning"]
                    }
                }
            }
        });
        Self::safe_api_call(client, api_key, &crate::model_config::get_light_models(), &payload)
            .await
    }

    pub async fn evaluate_evidence(
        client: &Client,
        api_key: &str,
        strategy: &str,
        action: &str,
        evidence: &str,
    ) -> Result<CriticResponse> {
        let truncated_evidence: String = evidence.chars().take(8000).collect();
        let prompt = format!("You are the Scientific Evaluator.\nSTRATEGY: {}\nACTION TAKEN: {}\nEVIDENCE GATHERED:\n{}\nDoes this evidence represent meaningful partial progress toward the strategy? If it gathers useful context (like listing a directory or reading a file), give a positive modifier. DO NOT demand the entire strategy be completed in one step.\nHALT RULE: If ACTION TAKEN is 'halt' and the EVIDENCE GATHERED (this turn or earlier in context) already shows a concrete success marker ([CMD_OK], [READ_OK], [WRITE_OK], [REPLACE_LINES_OK]) satisfying the strategy, this is a CORRECT completion, not a loss of progress -- give a positive modifier, not negative.\nMUTATION RULE: A successful mutating command (write, patch, mv, rm, mkdir, etc.) with [CMD_OK] and no stdout is genuine positive progress, not neutral reconnaissance -- do not score it 0.0 solely for lacking output.\nOUTPUT ONLY VALID JSON:\n{{\"critique\": \"<reason>\", \"confidence_modifier\": <float from -0.5 to 0.5>}}", strategy, action, truncated_evidence);
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
