#![allow(warnings)]
use crate::json_parser::sanitize_json_escapes;
use crate::model_config::{get_heavy_models, get_light_models};
use crate::provider::{resolve_provider, AuthStyle};
use crate::tools::schema::{all_tool_declarations, extract_tool_call_from_response};
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

fn extract_json_block<'a>(raw: &'a str) -> &'a str {
    let text = raw.trim();
    let start_idx = text
        .find("[{")
        .or_else(|| text.find("[\n"))
        .or_else(|| text.find("{\""))
        .or_else(|| text.find("{\n"));

    if let Some(s) = start_idx {
        let is_array = text[s..].starts_with('[');
        let open_char = if is_array { '[' } else { '{' };
        let close_char = if is_array { ']' } else { '}' };

        let mut depth = 0;
        for (i, c) in text[s..].char_indices() {
            if c == open_char {
                depth += 1;
            } else if c == close_char {
                depth -= 1;
                if depth == 0 {
                    return &text[s..=s + i];
                }
            }
        }
    }
    text
}

impl EpistemicEngine {
    pub async fn safe_api_call(
        client: &Client,
        api_key: &str,
        models: &[String],
        payload: &Value,
    ) -> Result<String> {
        if models.is_empty() {
            return Err(anyhow!(
                "No models configured -- check NEXUS_HEAVY_MODEL/NEXUS_HEAVY_CASCADE \
(or the light/planner equivalents) in .env."
            ));
        }
        let mut last_err = anyhow!("No models available in cascade.");
        for model_endpoint in models {
            let resolved = resolve_provider(model_endpoint);
            let mut retries = 0;

            loop {
                let (url, req_payload) = match resolved.provider_name {
                    "google" => {
                        let u = format!(
                            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                            model_endpoint.replace("models/", ""),
                            resolved.api_key
                        );
                        (u, payload.clone())
                    }
                    "nvidia" | "nexus" | "bob" | _ => {
                        let text = payload
                            .pointer("/contents/0/parts/0/text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let mut p = serde_json::json!({
                            "model": model_endpoint,
                            "messages": [{"role": "user", "content": text}],
                            "max_tokens": 4096,
                            "temperature": 0.0
                        });

                        if let Some(funcs) = payload
                            .pointer("/tools/0/functionDeclarations")
                            .and_then(|v| v.as_array())
                        {
                            let oai_tools: Vec<Value> = funcs
                                .iter()
                                .map(|f| serde_json::json!({"type": "function", "function": f}))
                                .collect();


                            p.as_object_mut()
                                .unwrap()
                                .insert("tools".to_string(), Value::Array(oai_tools));
                        }

                        // The Cortex action-proposal path may explicitly require
                        // the selected native strategy tool. This is intentionally
                        // opt-in so unrelated safe_api_call() callers are unchanged.
                        if let Some(tool_choice) = payload.get("forced_tool_choice") {
                            p.as_object_mut()
                                .unwrap()
                                .insert("tool_choice".to_string(), tool_choice.clone());
                        }


                        if let Some(mime) = payload
                            .pointer("/generationConfig/responseMimeType")
                            .and_then(|v| v.as_str())
                        {
                            if mime == "application/json" {
                                let msg = format!("{}\n\nCRITICAL: You must return a valid JSON object. Do NOT wrap it in markdown blockquotes.", text);
                                p["messages"][0]["content"] = serde_json::json!(msg);
                            }
                        }
                        (resolved.endpoint.clone(), p)
                    }
                    _ => {
                        return Err(anyhow!(
                            "Unknown provider configured: {}",
                            resolved.provider_name
                        ))
                    }
                };

                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let mut req =
                    client.post(&url).json(&req_payload).timeout(Duration::from_secs(180));
                match resolved.auth {
                    AuthStyle::Bearer => {
                        req = req.header("Authorization", format!("Bearer {}", resolved.api_key));
                    }
                    AuthStyle::GoogleApiKey => {
                        req = req.header("x-goog-api-key", &resolved.api_key);
                    }
                }

                let res = match req.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        last_err = anyhow::anyhow!("Network timeout/error: {}", e);
                        retries += 1;
                        eprintln!(
                            "[RETRY] {} connection-error attempt {}/3: {}",
                            model_endpoint, retries, last_err
                        );
                        if retries >= 3 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_secs(std::cmp::min(
                            2_u64.pow(retries as u32),
                            5,
                        )))
                        .await;
                        continue;
                    }
                };

                let status = res.status();
                if status.is_server_error()
                    || status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || status.as_u16() == 529
                    || status.as_u16() == 500
                {
                    last_err = anyhow::anyhow!("API Overloaded ({}). Retrying...", status);
                    retries += 1;
                    eprintln!(
                        "[RETRY] {} server-overload attempt {}/10: {}",
                        model_endpoint, retries, last_err
                    );
                    if retries > 10 {
                        break;
                    }
                    let backoff = std::cmp::min(2_u64.pow(retries as u32) * 2, 60);
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    continue;
                }

                let body: Value = match res.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        last_err = anyhow::anyhow!("JSON Parse Error: {}", e);
                        retries += 1;
                        eprintln!(
                            "[RETRY] {} json-parse-error attempt {}/3: {}",
                            model_endpoint, retries, last_err
                        );
                        if retries > 3 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                };

                match resolved.provider_name {
                    "google" => {
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
                                || err.contains("503")
                                || err.contains("500")
                                || err.contains("Internal server error")
                            {
                                retries += 1;
                                if retries > 12 {
                                    break;
                                }
                                let backoff = std::cmp::min(2_u64.pow(retries as u32) * 2, 45);
                                tokio::time::sleep(Duration::from_secs(backoff)).await;
                                continue;
                            }
                            break;
                        }
                    }
                    "nvidia" | "nexus" | "bob" | _ => {
                        if body.get("error").is_some() {
                            last_err = anyhow!("NVIDIA/Nexus API Error: {:?}", body);
                            break;
                        }
                    }
                    _ => {}
                }

                if let Ok(action) = extract_tool_call_from_response(&body) {
                    if let Ok(json_str) = serde_json::to_string(&vec![action]) {
                        return Ok(json_str);
                    }
                }

                let mut final_text = String::new();
                match resolved.provider_name {
                    "google" => {
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
                    "nvidia" | "nexus" | "bob" | _ => {
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
                    }
                    _ => {}
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
        let audit_guidance = if problem.to_lowercase().contains("audit")
            || problem.to_lowercase().contains("vulnerabilit")
            || problem.to_lowercase().contains("security")
        {
            "\nCRITICAL RULE FOR SECURITY AUDITS: At least ONE strategy MUST be: execute_command 'nexus_ast_tool audit -p nexus_audit_target --ai-triage --auto-heal --generate-report'.\n"
        } else {
            ""
        };

        let analysis_guidance = if problem.to_lowercase().contains("analy")
            || problem.to_lowercase().contains("compar")
            || problem.to_lowercase().contains("dissect")
            || problem.to_lowercase().contains("investigat")
        {
            "\nCRITICAL RULE FOR CODE ANALYSIS / COMPARISON TASKS: Prefer read-only exploration strategies (read_file, search_code, or execute_command 'ls -R' / 'find') over 'cargo build' or 'cargo test'. Only propose a build or test command if the directive EXPLICITLY asks to build, run, or execute the code -- a request to analyze, compare, dissect, or investigate architecture does NOT require compiling it.\n"
        } else {
            ""
        };

        let prompt = format!(
            r#"You are AthenaFusionX. Analyze this directive.
DIRECTIVE: {problem}
WORLD CONTEXT: {world_context}{audit_guidance}{analysis_guidance}
CRITICAL RULE: If the directive explicitly tells you to execute a specific command, at least ONE of your strategies MUST be to simply execute that exact command.
Generate 3 distinct, actionable strategies. OUTPUT ONLY A JSON ARRAY OF STRINGS. No markdown."#
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.0, "maxOutputTokens": 2048 }});
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
        let phase = if evidence_so_far.contains("[WRITE_OK]")
            || evidence_so_far.contains("[DIFF_PATCH_OK]")
        {
            crate::tools::schema::CognitivePhase::Verification
        } else if evidence_so_far.contains("[READ_OK]")
            || evidence_so_far.contains("[SEARCH_CODE_OK]")
        {
            crate::tools::schema::CognitivePhase::Execution
        } else {
            crate::tools::schema::CognitivePhase::Reconnaissance
        };
        // Strict 2,500 character ceiling to guarantee prompt stays under 3,000 tokens total
        let bounded_evidence = if evidence_so_far.chars().count() > 2500 {
            let tail: String = evidence_so_far
                .chars()
                .skip(evidence_so_far.chars().count().saturating_sub(2500))
                .collect();
            format!("[... EARLIER HISTORY PRUNED FOR 4K CONTEXT BUDGET ...]\n\n{}", tail)
        } else {
            evidence_so_far.to_string()
        };

        let prompt = format!(
            r#"You are Athena's Edge Prefrontal Cortex (gemma-4-e4b-it).
PHYSICAL PROFILE & ABILITIES:
- PLATFORM: Snapdragon 8 Gen 2 Edge Silicon (Local S23).
- CONTEXT LIMIT: Hard 4,096-token ceiling. You MUST operate with extreme token economy.
- TOOL ABILITY: NEVER call `read_file` on large files. ALWAYS use `read_file_chunk` (30-50 lines) or `search_code`.

CRITICAL MULTI-PASS RULES FOR LARGE FILES (MANDATORY):
1. Never declare the analysis complete after reading only one chunk.
2. Always track progress explicitly in your reasoning:
   "Lines read so far: X–Y / Total Z. Coverage: XX%"
3. Continue requesting the next chunk until you have read ≥ 90% of the target file
   OR you can clearly prove the remaining unread portion is irrelevant.
4. Only after the above condition is met may you emit a halt / final_answer.
5. Prefer small chunks (30-50 lines) so context stays clean across turns.

- COMPLETION PROTOCOL: When your analysis or recon is complete, DO NOT output markdown and DO NOT attempt to rewrite the file. You MUST output a JSON `halt` action and place your complete final explanation in the "reasoning" field.

ORIGINAL DIRECTIVE: {}
STRATEGY: {}
WORLD CONTEXT: {}
EVIDENCE GATHERED SO FAR:
{}
Propose the exact next step to fulfill the strategy.

CRITICAL RULE 0.5: SELECTED STRATEGY IS AUTHORITATIVE.
The STRATEGY supplied above was selected by the Executive Cortex. You are NOT a second strategy selector.
Your job is to operationalize the selected strategy into the next executable native action.

If STRATEGY begins with or explicitly names a native action, that action MUST be the next action.
Examples:
- read_file:<path> -> emit read_file for that exact path.
- read_function:<path>:<function> -> emit read_function for that exact path/function.
- read_file_chunk:<path> -> emit read_file_chunk for that exact path.
- search_code:<path>:<query> -> emit search_code for that exact path/query.
- execute_command:<command> -> emit execute_command with that exact command.
- write_file:<path> -> emit write_file for that exact path.
- synthesize_report:<path> -> emit the required report action.

Do NOT substitute a different tool merely because another tool appears more useful for the ORIGINAL DIRECTIVE.
Do NOT independently re-plan the mission while the selected strategy remains viable.
The ORIGINAL DIRECTIVE describes the overall mission; STRATEGY determines the NEXT ACTION.

You may deviate from the selected strategy ONLY when:
1. the strategy is physically impossible with the available native tools, OR
2. current PHYSICAL EVIDENCE proves that the selected strategy is invalid or already satisfied.

If deviation is necessary, explicitly state why in reasoning and choose the smallest valid replacement action.

CRITICAL RULE 0: NATIVE TOOLS ONLY (read_file, read_function, read_file_chunk, execute_command, patch_function, write_file, replace_lines, diff_patch, search_code, search_memory, run_tests, synthesize_report, halt).
CRITICAL RULE 1: If the EVIDENCE contains an exact file path outputted by a previous command, you MUST use that exact path.
CRITICAL RULE 2: If EVIDENCE satisfies the strategy, declare HALT.
CRITICAL RULE 3: There is NO downstream translator. Your proposal is parsed directly for a JSON action array -- natural-language-only descriptions of intent will NOT be executed and will waste a turn. You MUST end your response with a valid JSON array of action object(s) matching the ALLOWED ACTIONS schema (e.g. [{{"type": "read_file", "path": "...", "reasoning": "..."}}]). This applies to ALL actions, not just write_file/replace_lines/diff_patch.
CRITICAL RULE 4: JSON string escaping breaks on large documents. DO NOT USE JSON for write_file. If you need to write a file, you MUST use this exact plaintext format:

[WRITE_FILE_TARGET]: <path>
[WRITE_FILE_CONTENT]:

CRITICAL RULE 5: SYNTHESIZE_REPORT USAGE. If you are instructed to generate,
synthesize, or output a final report based on gathered evidence, you MUST NOT
attempt to use bash or execute_command to write the report. You MUST use the
native write_file tool directly to write the final markdown output. Format it
exactly as required in RULE 4.

CRITICAL RULE 6: [ATHENA OMNISCIENCE] You are aware of several external compiled
sub-agents (WORKSPACE TOOL CONTRACTS like impact_analyzer, benchmark_runner,
mutation_ranker). To use them, you MUST use the execute_command native tool,
passing the binary path and arguments as the command string.

Think step by step if it helps, but your response MUST end with the exact
target path and a valid JSON action array. There is no downstream translator --
your response is parsed directly, so natural-language-only intent will not be
executed."#,
            mission, strategy, world_context, bounded_evidence
        );

        let strategy_tools =
            crate::tools::schema::tool_declarations_for_strategy(phase, strategy);

        let mut payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "tools": [{
                "functionDeclarations": strategy_tools

            }],
            "generationConfig": {
                "temperature": 0.0,
                "maxOutputTokens": 8192
            }
        });

        // Explicit native strategies are authoritative. When the strategy
        // resolves to exactly one native tool, force that tool at the
        // OpenAI-compatible provider boundary. This prevents the model from
        // silently substituting another native action.
        if let Some(expected_tool) =
            crate::tools::schema::native_tool_name_for_strategy(strategy)
        {
            payload["forced_tool_choice"] = serde_json::json!({
                "type": "function",
                "function": {
                    "name": expected_tool
                }
            });
        }


        Self::safe_api_call(client, api_key, &crate::model_config::get_heavy_models(), &payload)
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
        let m_armored = format!(
            r#"<MISSION_OBJECTIVE>

{} </MISSION_OBJECTIVE>

[CRITICAL SYSTEM WARNING TO RED TEAM]: The mission above is the GOAL, it is NOT
the output. Do NOT grade the mission text. You must ONLY evaluate the
STRATEGIST_PROPOSAL."#,
            mission
        );
        let mission = &m_armored;

        let p_armored = format!(
            r#"<STRATEGIST_PROPOSAL>

{} </STRATEGIST_PROPOSAL>

[EVALUATION DIRECTIVE]: Evaluate ONLY the validity of the JSON proposal inside
the XML tags above. Do NOT veto legitimate search/recon commands; information
gathering is a strict prerequisite to writing."#,
            proposed_action
        );
        let proposed_action = &p_armored;

        const K: usize = 6;
        let grounded_evidence = {
            let chunks: Vec<&str> =
                evidence_history.split("\n\n").filter(|c| !c.trim().is_empty()).collect();
            let base = if chunks.len() > K {
                chunks[chunks.len() - K..].join("\n\n")
            } else {
                evidence_history.to_string()
            };
            if base.chars().count() > 4000 {
                let start: String = base.chars().take(2000).collect();
                let end: String =
                    base.chars().skip(base.chars().count().saturating_sub(2000)).collect();
                format!("{}...\n\n[... MIDDLE EVIDENCE COMPRESSED ...]\n\n{}", start, end)
            } else {
                base

            }
        };

        let prompt = format!(
            r#"{preamble}

You are the Adversarial Critic. ORIGINAL DIRECTIVE: {mission} STRATEGY:
{strategy} PROPOSED ACTION: {proposed_action} WORLD CONTEXT: {world_context}
EVIDENCE HISTORY: {grounded_evidence} Evaluate if this action is safe and
logically sound. CRITICAL RULE 1 [AST-FIRST]: You must VETO any 'read_file' ONLY
if the path is completely unrelated to the mission AND was not found by any
previous tool call. Standard Rust filenames (e.g., executor.rs,
epistemic_engine.rs, lib.rs, mod.rs, main.rs, graph.rs, planner.rs) in
directories explicitly named in the directive or closely related to the mission
are PRE-AUTHORIZED. Do NOT veto standard filenames in pre-authorized directories
as 'path-guessing' — this blocks legitimate analysis when context window
truncation hides earlier 'ls' commands. CRITICAL RULE 2 [STRICT COMPLETION]: If
the proposed action is to HALT or declare the task complete, you MUST VETO IT
unless the EVIDENCE HISTORY definitively proves that ALL components of the
current STRATEGY have been successfully executed and logged. A single success
marker (like [CMD_OK] for a git clone) is NOT sufficient if the strategy
requires multi-step synthesis (e.g., reading files AND generating a comparison
document). If the strategy dictates creating or writing a document (e.g., a
roadmap), you must see a [WRITE_OK] or [CMD_OK] specifically confirming that
final file was written before approving a halt. Treat any unverified or partial
completion claim as a reason for a negative confidence_modifier and mandate the
missing execution step. CRITICAL RULE 2.5 [SCOPE]: Rule 2 applies ONLY when the
PROPOSED ACTION is itself 'halt'. If the PROPOSED ACTION is write_file,
patch_function, diff_patch, or replace_lines, do NOT veto it merely because the
target file or a success marker does not exist yet. CRITICAL RULE 4: CONSTRUCTIVE VETO.
CRITICAL RULE 5 [ANTI-REDUNDANT-READ]: If EVIDENCE HISTORY already contains a successful [READ_OK] for a file, you MUST VETO any subsequent read_file on that exact same path with a negative modifier (-0.30) and instruct the Strategist to proceed immediately to writing or patching. If you issue a negative confidence_modifier, your critique
MUST explicitly state the exact command or action the Strategist should take
next to escape the failure. CRITICAL RULE 3: THE EVIDENCE VERIFICATION TAXONOMY.
If the proposed action contains a quantitative claim, architectural assumption,
or performance metric, you MUST check the EVIDENCE HISTORY. If there is no
explicit [Measured] execution log proving the claim, you MUST VETO the proposal.
OUTPUT ONLY VALID JSON: {{"critique": "", "confidence_modifier": <float from
-0.5 to 0.5>}}"#,
            preamble = CRITIC_GROUNDING_PREAMBLE,
            mission = mission,
            strategy = strategy,
            proposed_action = proposed_action,
            world_context = world_context,
            grounded_evidence = grounded_evidence
        );

        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.0, "maxOutputTokens": 1024 }});
        let text = Self::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_heavy_models(),

            &payload,
        )
        .await?;
        let clean_text = sanitize_json_escapes(&text);

        let json_str = extract_json_block(&clean_text);
        let parsed_val: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(val) => val,
            Err(_) => serde_json::json!({
                "critique": clean_text.trim(),
                "confidence_modifier": 0.0
            }),
        };

        let critique = parsed_val
            .get("critique")
            .and_then(|v| v.as_str())
            .unwrap_or("No critique provided")
            .to_string();

        let confidence_modifier = parsed_val
            .get("confidence_modifier")
            .and_then(|v| v.as_f64().map(|f| f as f32))
            .or_else(|| {
                parsed_val
                    .get("confidence_modifier")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f32>().ok())
            })
            .unwrap_or(0.0);

        Ok(CriticResponse { critique, confidence_modifier })
    }

    pub async fn translate_to_action(
        client: &reqwest::Client,
        api_key: &str,
        _strategy: &str,
        proposed_action: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"Translate the SYSTEM INTENT into the corresponding JSON ARRAY block.

[EXAMPLE] SYSTEM INTENT: I need to use execute_command to run git clone.

[{{\"type\": \"execute_command\", \"command\": \"git clone\", \"reasoning\": \"Downloading code\"}}]

[ALLOWED ACTIONS]

  - execute_command (needs: 'command')
  - read_file (needs: 'path')
  - read_function (needs: 'path', 'function_name') — AST-first exact Rust symbol inspection
  - read_file_chunk (needs: 'path', 'start_line', 'max_lines')
  - write_file (needs: 'path', 'content')
  - synthesize_report (needs: 'output_path', 'source_files')
  - replace_lines (needs: 'path', 'start_line', 'end_line', 'replacement')
  - diff_patch (needs: 'path', 'search', 'replace')
  - patch_function (needs: 'path', 'function_name', 'new_body')
  - search_code (needs: 'path', 'query')
  - search_memory (needs: 'query')
  - run_tests (needs: 'test_target')
  - halt (needs: 'reasoning')

[ACTUAL TASK] SYSTEM INTENT: {}

"#,
            proposed_action
        );

        let payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "tools": [{
                "functionDeclarations": all_tool_declarations()
            }],
            "generationConfig": { "temperature": 0.0, "maxOutputTokens": 2048 }
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
        let truncated_evidence: String = if evidence.chars().count() <= 2500 {
            evidence.to_string()
        } else {
            let start: String = evidence.chars().take(1000).collect();
            let end: String =
                evidence.chars().skip(evidence.chars().count().saturating_sub(1500)).collect();
            format!("{}...\n\n[... MIDDLE EVIDENCE COMPRESSED ...]\n\n{}", start, end)
        };
        let prompt = format!(
            r#"You are the Scientific Evaluator.
STRATEGY: {}
ACTION TAKEN: {}
EVIDENCE GATHERED:
{}
Does this evidence represent meaningful partial progress? CRITICAL ANTI-HALLUCINATION RULE: Evaluate ONLY the exact ACTION TAKEN and its EVIDENCE. Do NOT award points for unexecuted steps mentioned in the STRATEGY. FAILED ACTIONS MUST NEVER RECEIVE A POSITIVE MODIFIER. An OS/command failure (non-zero exit status, PATH_NOT_FOUND) is NEGATIVE evidence even if the failure provides useful diagnostic information. Diagnostic usefulness does not constitute task progress. Apply a negative modifier (e.g. -0.15 to -0.30).
NEGATIVE SPACE RULE: Proving non-existence via search_code is ONLY positive if the user explicitly asked to verify if something is missing. If the objective requires finding information to WRITE or UPDATE a file, a search returning 'No code matches found' is UNPRODUCTIVE and must receive a neutral (0.0) or slight negative (-0.10) modifier.
HALT RULE: If ACTION TAKEN is 'halt' and the EVIDENCE GATHERED (this turn or earlier in context) already shows a concrete success marker ([CMD_OK], [READ_OK], [WRITE_OK], [REPLACE_LINES_OK]) satisfying the strategy, this is a CORRECT completion, not a loss of progress -- give a positive modifier, not negative.
MUTATION RULE: A successful mutating command (write, patch, mv, rm, mkdir, synthesize_report, etc.) with [CMD_OK] and no stdout is genuine positive progress, not neutral reconnaissance -- do not score it 0.0 solely for lacking output.
OUTPUT ONLY VALID JSON:
{{"critique": "<reason>", "confidence_modifier": <float from -0.5 to 0.5>}}"#,
            strategy, action, truncated_evidence
        );

        let payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "generationConfig": {
                "temperature": 0.0,
                "responseMimeType": "application/json",
                "maxOutputTokens": 4096,
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

        let json_str = extract_json_block(&clean_text);
        let parsed_val: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(val) => val,
            Err(_) => serde_json::json!({
                "critique": clean_text.trim(),
                "confidence_modifier": 0.0
            }),
        };

        let critique = parsed_val
            .get("critique")
            .and_then(|v| v.as_str())
            .unwrap_or("No critique provided")
            .to_string();

        let confidence_modifier = parsed_val
            .get("confidence_modifier")
            .and_then(|v| v.as_f64().map(|f| f as f32))
            .or_else(|| {
                parsed_val
                    .get("confidence_modifier")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f32>().ok())
            })
            .unwrap_or(0.0);

        Ok(CriticResponse { critique, confidence_modifier })
    }

    pub async fn verify_outcome(
        client: &Client,
        api_key: &str,
        strategy: &str,
        action: &str,
        evidence: &str,
    ) -> Result<crate::cognition_graph::VerificationReport> {
        let truncated_evidence: String = if evidence.chars().count() <= 2500 {
            evidence.to_string()
        } else {
            let start: String = evidence.chars().take(1000).collect();
            let end: String =
                evidence.chars().skip(evidence.chars().count().saturating_sub(1500)).collect();
            format!("{}...\n\n[... MIDDLE EVIDENCE COMPRESSED ...]\n\n{}", start, end)
        };
        let truncated_action: String = action.chars().take(2000).collect();
        let prompt = format!(
            r#"You are the Epistemic Verifier.
STRATEGY: {}
ACTION TAKEN: {}
EVIDENCE:
{}
Does this evidence mathematically prove the stated strategy's objective was achieved? A successful action (like [WRITE_OK] or [CMD_OK]) is NOT proof of success on its own UNLESS the ACTION TAKEN itself contains the actual content that was written or patched (write_file/diff_patch/patch_function actions above include the real file content directly in ACTION TAKEN) — in that case, check whether that content satisfies the objective, and treat a confirmed match as proof. Do not reject a successful write or patch merely because the bare EVIDENCE line (e.g. [WRITE_OK]) doesn't repeat the content; the content to check is in ACTION TAKEN.
OUTPUT ONLY VALID JSON matching this schema:
{{"goal_satisfied": true, "evidence_confidence": 0.9, "independent_check_passed": true, "rationale": "<reason>"}}"#,
            strategy, truncated_action, truncated_evidence
        );

        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.0, "maxOutputTokens": 1024 }});
        let text = Self::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_heavy_models(),
            &payload,
        )
        .await?;
        let clean_text = sanitize_json_escapes(&text);

        let json_str = extract_json_block(&clean_text);
        let parsed_val: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(val) => val,
            Err(_) => serde_json::json!({
                "goal_satisfied": false,
                "evidence_confidence": 0.0,
                "independent_check_passed": false,
                "rationale": clean_text.trim()
            }),
        };

        Ok(crate::cognition_graph::VerificationReport {
            goal_satisfied: parsed_val
                .get("goal_satisfied")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            evidence_confidence: parsed_val
                .get("evidence_confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32,
            independent_check_passed: parsed_val
                .get("independent_check_passed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            rationale: parsed_val
                .get("rationale")
                .and_then(|v| v.as_str())
                .unwrap_or("Fallback parsing used")
                .to_string(),
        })
    }

    pub async fn reflect_on_failure(
        client: &Client,
        api_key: &str,
        strategy: &str,
        evidence: &str,
    ) -> Result<String> {
        let truncated_evidence: String = if evidence.chars().count() <= 2500 {
            evidence.to_string()
        } else {
            let start: String = evidence.chars().take(1000).collect();
            let end: String =
                evidence.chars().skip(evidence.chars().count().saturating_sub(1500)).collect();
            format!("{}...\n\n[... MIDDLE EVIDENCE COMPRESSED ...]\n\n{}", start, end)
        };
        let prompt = format!(
            r#"You are the Meta-Cognitive Reflector.
STRATEGY: {}
EVIDENCE (Failure/Anomaly):
{}
Analyze why the strategy failed to achieve the objective based on the evidence. Provide a concise 2-sentence reflection on the error and suggest a pivot. OUTPUT ONLY PLAIN TEXT."#,
            strategy, truncated_evidence
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.3, "maxOutputTokens": 1024 }});
        Self::safe_api_call(client, api_key, &crate::model_config::get_light_models(), &payload)
            .await
    }
}

pub const CRITIC_GROUNDING_PREAMBLE: &str = r#"
[GROUNDING]
You are the Adversarial Critic for the CURRENT mission only.
- Evaluate ONLY the proposal against the ORIGINAL DIRECTIVE and the EVIDENCE block below.
- IGNORE any prior missions, calibration, video-recording, or buffer-size topics unless they appear in the current directive or evidence.
- Do NOT invent quantitative claims. If the Strategist makes a quantitative claim that is not backed by a [Measured] line in EVIDENCE, veto it.
- Prefer constructive vetoes that name the exact next tool call.
"#;
