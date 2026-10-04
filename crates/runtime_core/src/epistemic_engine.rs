#![allow(warnings)]
use crate::json_parser::sanitize_json_escapes;
use crate::provider::{resolve_provider, AuthStyle};
use crate::tools::schema::{all_tool_declarations, extract_tool_call_from_response};
use anyhow::{anyhow, Result};
use nexus_strategy_algorithms::{DirectiveInputs, DirectiveStrategyCompiler};
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
        eprintln!("[MODEL TRACE] safe_api_call models={:?}", models);

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
                eprintln!(
                    "[MODEL TRACE] -> attempt model={} provider={} retry={}",
                    model_endpoint, resolved.provider_name, retries
                );
                let (url, req_payload) = match resolved.provider_name {
                    "google" => {
                        let u = format!(
                            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                            model_endpoint.replace("models/", ""),
                            resolved.api_key
                        );

                        // Gemini's generateContent endpoint rejects unknown
                        // top-level fields with 400 INVALID_ARGUMENT. The
                        // forced_tool_choice hint is an OpenAI-shaped extension
                        // consumed only by the NVIDIA/Nexus branch below; it
                        // must not leak into the Gemini payload.
                        //
                        // The strategist prompt already names the selected
                        // native tool, and the response parser
                        // (extract_tool_call_from_response) accepts Gemini's
                        // native functionCall shape, so the hint is advisory
                        // on this provider and can be dropped.
                        //
                        // If Gemini ever needs a hard constraint, translate
                        // instead to:
                        //   tool_config.function_calling_config.mode = "ANY"
                        //   tool_config.function_calling_config.allowed_function_names = [name]
                        // That is a behavioral change and is NOT done here.
                        let mut p = payload.clone();
                        if let Some(obj) = p.as_object_mut() {
                            obj.remove("forced_tool_choice");
                        }
                        (u, p)
                    }
                    "nvidia" | "nexus" | "bob" | _ => {
                        let text = payload
                            .pointer("/contents/0/parts/0/text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let requested_max_tokens = payload
                            .pointer("/generationConfig/maxOutputTokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(4096);

                        let mut p = serde_json::json!({
                            "model": model_endpoint,
                            "messages": [{"role": "user", "content": text}],
                            "max_tokens": requested_max_tokens,
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
                            // Only force a tool the request actually declares;
                            // otherwise the API rejects it with a 400.
                            let forced_name = tool_choice
                                .pointer("/function/name")
                                .and_then(|v| v.as_str())
                                .or_else(|| tool_choice.as_str());
                            let declared = p
                                .get("tools")
                                .and_then(|t| t.as_array())
                                .map(|arr| {
                                    arr.iter().any(|t| {
                                        t.pointer("/function/name").and_then(|v| v.as_str())
                                            == forced_name
                                    })
                                })
                                .unwrap_or(false);
                            if declared || forced_name.is_none() {
                                p.as_object_mut()
                                    .unwrap()
                                    .insert("tool_choice".to_string(), tool_choice.clone());
                            }
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
                        // Reasoning control for NVIDIA NIM. GLM ignores
                        // chat_template_kwargs and needs top-level
                        // reasoning_effort; others use the template kwargs.
                        if resolved.provider_name == "nvidia" {
                            if model_endpoint.to_ascii_lowercase().starts_with("z-ai/") {
                                p["reasoning_effort"] = serde_json::json!("low");
                            } else {
                                p["chat_template_kwargs"] = serde_json::json!({
                                    "enable_thinking": false,
                                    "thinking": false,
                                    "force_nonempty_content": true
                                });
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

                let finish_reason = match resolved.provider_name {
                    "google" => body
                        .pointer("/candidates/0/finishReason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    _ => body
                        .pointer("/choices/0/finish_reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                };

                if finish_reason == "MAX_TOKENS" || finish_reason == "length" {
                    last_err = anyhow!(
                        "{} response truncated ({}) after {} chars -- retrying",
                        model_endpoint,
                        finish_reason,
                        final_text.len()
                    );
                    retries += 1;
                    eprintln!(
                        "[RETRY] {} truncated-response attempt {}/4: {}",
                        model_endpoint, retries, last_err
                    );
                    if retries > 4 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(2_u64.pow(retries as u32) * 2)).await;
                    continue;
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

    /// Restrict deterministic directive parsing to the node-local portion
    /// of task_manager.rs's combined_mission template.
    ///
    /// ROOT DIRECTIVE CONTEXT and LINEAGE CONTEXT are background context and
    /// must not influence deterministic strategy selection for this node.
    /// When those headers are absent, preserve direct-call behavior by
    /// returning the complete mission unchanged.
    fn scope_to_node_instructions(mission: &str) -> &str {
        let end = mission
            .find("\nROOT DIRECTIVE CONTEXT:")
            .or_else(|| mission.find("\nLINEAGE CONTEXT:"))
            .unwrap_or(mission.len());

        &mission[..end]
    }

    /// Prompt-window `evidence` while keeping physically read source whole.
    ///
    /// RULE 6 and the verifier's quote clause require the judging model to
    /// SEE the source a halt's claims are about; naive head/tail windows
    /// elide the middle of a large [READ_OK] ... FULL FILE CONTENT block --
    /// exactly where the interesting lines live -- so grounded claims fail
    /// for lack of visible quotes. This windows only the non-read material;
    /// every read-record region survives verbatim and in order.
    /// Region boundaries use a POSITIVE list of known evidence markers: an
    /// unknown bracketed line (e.g. a source line starting with `[ERROR]`)
    /// does NOT end the pinned region -- when in doubt we over-pin, never
    /// silently drop source. Oversized regions are capped with a VISIBLE
    /// marker, never silently, because the critic may run on a small-context
    /// tier. The strategist keeps its own strict 2500-char edge budget on
    /// purpose (4K-context persona); only judgment layers pin reads.
    pub(crate) fn compress_preserving_reads(evidence: &str, head: usize, tail: usize) -> String {
        const MAX_PINNED_REGION: usize = 12_000;

        let starts_read_region = |line: &str| {
            let t = line.trim_start();
            t.starts_with("[READ_OK]") || t.starts_with("[READ_CHUNK_OK]")
        };
        let ends_region = |line: &str| {
            const ENDERS: &[&str] = &[
                "[READ_OK]",
                "[READ_CHUNK_OK]",
                "[WRITE_OK]",
                "[CMD_OK]",
                "[SEARCH_CODE_OK]",
                "[SEARCH_MEMORY_OK]",
                "[PATCH_FUNCTION_OK]",
                "[PATCH_IDEMPOTENT_OK]",
                "[DIFF_PATCH_OK]",
                "[REPLACE_LINES_OK]",
                "[RUN_TESTS_OK]",
                "[COMPILE_OK]",
                "[PROPOSITION]",
                "[HALT",
                "[CRITIC_VETO]",
                "[CRITIC_FACT_OVERRIDE]",
                "[HALT_REJECTED]",
                "[PHYSICAL SESSION EVIDENCE]",
            ];
            let t = line.trim_start();
            ENDERS.iter().any(|m| t.starts_with(m)) && !starts_read_region(t)
        };

        enum Seg {
            Pinned(String),
            Plain(String),
        }

        let lines: Vec<&str> = evidence.lines().collect();
        let mut segs: Vec<Seg> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if starts_read_region(lines[i]) {
                let mut region = String::new();
                while i < lines.len() {
                    let cur = lines[i];
                    if !region.is_empty() && ends_region(cur) {
                        break;
                    }
                    region.push_str(cur);
                    region.push('\n');
                    i += 1;
                }
                let pinned = if region.chars().count() > MAX_PINNED_REGION {
                    let kept: String = region.chars().take(MAX_PINNED_REGION).collect();
                    format!(
                        "{}\n[... READ BODY CAPPED AT {} CHARS IN JUDGE WINDOW ...]",
                        kept, MAX_PINNED_REGION
                    )
                } else {
                    region.trim_end_matches('\n').to_string()
                };
                segs.push(Seg::Pinned(pinned));
            } else {
                segs.push(Seg::Plain(lines[i].to_string()));
                i += 1;
            }
        }

        let plain_total: usize = segs
            .iter()
            .map(|s| match s {
                Seg::Plain(l) => l.len() + 1,
                Seg::Pinned(_) => 0,
            })
            .sum();
        let render = |s: &Seg| match s {
            Seg::Plain(l) => l.clone(),
            Seg::Pinned(r) => r.clone(),
        };
        if plain_total <= head + tail {
            return segs.iter().map(render).collect::<Vec<_>>().join("\n");
        }

        let n = segs.len();
        let mut head_end = 0usize;
        let mut used = 0usize;
        while head_end < n && used < head {
            if let Seg::Plain(l) = &segs[head_end] {
                used += l.len() + 1;
            }
            head_end += 1;
        }
        let mut tail_start = n;
        used = 0;
        while tail_start > head_end && used < tail {
            if let Seg::Plain(l) = &segs[tail_start - 1] {
                used += l.len() + 1;
            }
            tail_start -= 1;
        }

        let mut out: Vec<String> = Vec::new();
        for s in &segs[..head_end] {
            out.push(render(s));
        }
        let middle = &segs[head_end..tail_start];
        if middle.iter().any(|s| matches!(s, Seg::Plain(_))) {
            out.push("[... MIDDLE EVIDENCE COMPRESSED ...]".to_string());
        }
        for s in middle {
            if matches!(s, Seg::Pinned(_)) {
                out.push(render(s));
            }
        }
        for s in &segs[tail_start..] {
            out.push(render(s));
        }
        out.join("\n")
    }

    /// Evidence history as the CRITIC may see it. Prior critic verdicts and
    /// halt rejections are feedback for the STRATEGIST, not evidence for the
    /// critic: keeping them let the critic restate its previous verdict as a
    /// fresh one (observed echo: a veto about "no defect found" issued
    /// against a proposal that DID report a defect).
    pub(crate) fn critic_visible_history(evidence_history: &str) -> String {
        evidence_history
            .split("\n\n")
            .filter(|c| {
                !c.trim().is_empty()
                    && !c.contains("[CRITIC_VETO]")
                    && !c.contains("[HALT_REJECTED]")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub async fn generate_hypotheses(
        client: &Client,
        api_key: &str,
        problem: &str,
        world_context: &str,
    ) -> Result<Vec<String>> {
        // DETERMINISTIC-COMPILER SCOPING:
        //
        // task_manager.rs embeds the complete original directive under
        // "ROOT DIRECTIVE CONTEXT:" and lineage information under
        // "LINEAGE CONTEXT:". Those sections are background context for the
        // probabilistic model and are NOT this node's instructions.
        //
        // The deterministic compiler must operate only on the node-local
        // OBJECTIVE / ROLE / SUCCESS CRITERIA / COMPLETION RULE section.
        // Otherwise a later step such as "use write_file to create X" can
        // contaminate an earlier node whose actual objective is to clone or
        // inspect a repository.
        //
        // The full `problem` remains available below for the probabilistic
        // prompt, where the broader mission context is intentionally useful.
        let scoped_problem = Self::scope_to_node_instructions(problem);

        // EXPLICIT NATIVE TOOL CONTRACT:
        // If the original directive explicitly names SearchMemory, the
        // Executive Cortex must not reinterpret the mission as filesystem
        // exploration, code search, or host observation. Preserve the
        // directive's requested native operation as the authoritative strategy.
        let problem_lower = scoped_problem.to_lowercase();

        // Inline-analysis nodes (ANALYSIS_OK): the code is in the directive.
        // The only valid strategy is to deliver the answer via halt.
        let has_inline_code = problem.contains("fn ")
            || problem.contains("```")
            || problem.contains("struct ")
            || problem.contains("enum ")
            || problem.contains("impl ");
        if scoped_problem.contains("ANALYSIS_OK")
            && has_inline_code
            && !scoped_problem.contains("READ_OK:")
            && !scoped_problem.contains("SEARCH_CODE_OK:")
        {
            return Ok(vec![
                "halt: the material to analyze is inline in the directive; deliver the complete written analysis in one halt action with no searches, reads, or commands".to_string(),
            ]);
        }

        let has_explicit_success_criterion = problem_lower.contains("search_memory_ok:")
            || problem_lower.contains("search_code_ok:")
            || problem_lower.contains("read_ok:")
            || problem_lower.contains("write_ok:")
            || problem_lower.contains("read_function_ok:")
            || problem_lower.contains("read_file_chunk_ok:")
            || problem_lower.contains("patch_function_ok:")
            || problem_lower.contains("diff_patch_ok:")
            || problem_lower.contains("replace_lines_ok:")
            || problem_lower.contains("cmd_ok")
            || problem_lower.contains("dag_lineage_ok")
            || problem_lower.contains("node_objective_executed_and_verified");

        let search_memory_explicit = !has_explicit_success_criterion
            && (crate::completion_engine::any_word_unnegated(&problem_lower, &["searchmemory"])
                || crate::completion_engine::any_word_unnegated(
                    &problem_lower,
                    &["search_memory"],
                )
                || (crate::completion_engine::any_word_unnegated(&problem_lower, &["search"])
                    && crate::completion_engine::any_word_unnegated(&problem_lower, &["memory"])));

        let read_targets =
            crate::completion_engine::extract_all_named_targets(scoped_problem, "READ_OK:");

        let required_command = crate::completion_engine::extract_required_command(scoped_problem);

        let lineage_inherit_only =
            crate::completion_engine::is_lineage_inherit_only_objective(scoped_problem);

        let host_observation =
            crate::completion_engine::is_host_observation_objective(scoped_problem);

        let directive_inputs = DirectiveInputs {
            problem: scoped_problem.to_string(),
            problem_lower: problem_lower.clone(),
            has_explicit_success_criterion,
            search_memory_explicit,
            read_targets,
            required_command,
            lineage_inherit_only,
            host_observation,
        };

        if let Some(strategies) = DirectiveStrategyCompiler::compile(&directive_inputs) {
            return Ok(strategies);
        }

        // PROBABILISTIC-PROMPT SCOPING:
        //
        // The deterministic compiler above already operates on scoped_problem.
        // Everything past this point -- audit_guidance/analysis_guidance
        // detection and the LLM strategy-generation prompt itself -- must
        // ALSO see only this node's own instructions, not the full
        // combined_mission (which still carries every OTHER step of a
        // multi-step directive under ROOT DIRECTIVE CONTEXT). Without this,
        // a node whose deterministic strategy could not be derived (e.g.
        // extract_required_command failing to parse an unquoted "run
        // exactly: <cmd>" phrasing) falls through to the LLM, which then
        // sees the entire multi-step mission and invents one strategy per
        // overall mission step instead of per-node alternatives. Confirmed
        // root cause of the OpenFang-roadmap mission's task_1 oscillation:
        // task_1's own job was only the git clone, but its turn-1
        // hypothesis set included a `search_code` strategy (task_2's job)
        // and a `write_file` strategy (task_3's job), which then competed
        // with the real clone hypothesis turn after turn.
        //
        // Shadowing `problem` here scopes every remaining reference in this
        // function (audit_guidance, analysis_guidance, the prompt's
        // DIRECTIVE line, and the final "Execute directive: {}" fallback)
        // without editing each call site individually.
        let problem = scoped_problem;

        let audit_guidance = if problem_lower.contains("audit")
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
        let models = match nexus_algorithms::Router::select("reasoning") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };
        let text = Self::safe_api_call(client, api_key, &models, &payload).await?;
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

    /// Extract an imperative `(path, function_name)` patch target.
    ///
    /// This is intentionally narrow: it requires a patch directive plus a
    /// source path and function identifier. It exists to distinguish the
    /// mutation phase of compound missions from a later verification command.
    fn extract_patch_target(problem: &str, problem_lower: &str) -> Option<(String, String)> {
        let has_patch_verb = problem_lower.contains("patch_function")
            || problem_lower.contains("patch `")
            || problem_lower.contains("patch the")
            || problem_lower.contains("patch function");

        if !has_patch_verb {
            return None;
        }

        let path = problem
            .split_whitespace()
            .map(|tok| {
                tok.trim_matches(|c: char| {
                    !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
                })
            })
            .find(|tok| {
                tok.contains('/')
                    && (tok.ends_with(".rs")
                        || tok.ends_with(".py")
                        || tok.ends_with(".js")
                        || tok.ends_with(".ts")
                        || tok.ends_with(".php"))
            })?;

        // Prefer the identifier explicitly following the word "function".
        let words: Vec<&str> = problem.split_whitespace().collect();

        for (i, word) in words.iter().enumerate() {
            let normalized =
                word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_lowercase();

            if normalized == "function" && i + 1 < words.len() {
                let candidate =
                    words[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                if !candidate.is_empty()
                    && candidate.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    return Some((path.to_string(), candidate.to_string()));
                }
            }
        }

        // Fallback for compact directives such as:
        // "patch add in apps/math_utils/src/lib.rs"
        for (i, word) in words.iter().enumerate() {
            let normalized =
                word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_lowercase();

            if normalized == "patch" && i + 1 < words.len() {
                let candidate =
                    words[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                if !candidate.is_empty()
                    && candidate.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && candidate != "the"
                    && candidate != "function"
                    && candidate != "so"
                    && candidate != "to"
                {
                    return Some((path.to_string(), candidate.to_string()));
                }
            }
        }

        None
    }

    pub async fn propose_action(
        client: &Client,
        api_key: &str,
        mission: &str,
        strategy: &str,
        world_context: &str,
        evidence_so_far: &str,
    ) -> Result<String> {
        // An explicit native write strategy is an execution operation even
        // when prior evidence has already advanced the session into the
        // verification phase. Verification must not perform mutations, and
        // therefore write_file must be selected from the Execution phase.
        let phase = if strategy.starts_with("write_file:")
            || strategy.starts_with("diff_patch:")
            || strategy.starts_with("patch_function:")
            || strategy.starts_with("replace_lines:")
            || strategy.starts_with("synthesize_report:")
        {
            crate::tools::schema::CognitivePhase::Execution
        } else if evidence_so_far.contains("[WRITE_OK]")
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

- COMPLETION PROTOCOL: When your analysis or recon is complete, DO NOT output markdown and DO NOT attempt to rewrite the file. You MUST output a JSON `halt` action and place your complete final explanation in the "reasoning" field. If the mission specifies an output format (for example ROOT_CAUSE / TRACE / FIX), the reasoning field MUST use exactly those labeled sections, each containing concrete findings with file and line references; never write only a statement that the analysis is complete.

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
- search_memory:<query> -> emit search_memory with that exact query.
- read_function:<path>:<function> -> emit read_function for that exact path/function.
- search_code:<path>:<query> -> emit search_code with `path` EXACTLY equal to <path> and `query` EXACTLY equal to <query>.
  IMPORTANT: <path> is the search scope (directory or file) and <query> is the text/pattern to search for.
  NEVER append <query> to <path>. NEVER convert `search_code:<path>:<query>` into `<path>/<query>`.
  Example: `search_code:vendor/openfang:HAND.toml` MUST become
  `[{{"type":"search_code","path":"vendor/openfang","query":"HAND.toml","reasoning":"scope search for the criterion term"}}]`.
  IMPORTANT: for a `search_code:<dir>:<query>` strategy, `path` MUST be
  exactly `<dir>` and MUST NOT be extended with a filename, subdirectory,
  or file extension. The `query` is the substring to search for and MAY
  itself look like a filename or path -- when it does, search the whole
  scope and let the pattern match any file recursively. Example: for the
  strategy `search_code:vendor/openfang:HAND.toml`, the correct action is
  {{"type":"search_code","path":"vendor/openfang","query":"HAND.toml"}}.
  Wrong: {{"type":"search_code","path":"vendor/openfang/HAND.toml","query":"HAND.toml"}}.
- read_file_chunk:<path> -> emit read_file_chunk for that exact path.
- read_file:<path> -> emit read_file for that exact path.
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

CRITICAL PATCH_FUNCTION RULE: When the selected STRATEGY begins with `patch_function:`, the emitted patch_function action MUST make a real, mission-compliant mutation. Do NOT return the existing function implementation unchanged merely because its computational behavior already matches part of the directive. Preserve the inherited function signature and satisfy ALL requirements in the ORIGINAL DIRECTIVE, including documentation, comments, cleanup, or other explicitly requested implementation changes. The `new_body` must contain the complete replacement function definition/body required by the native patch_function contract. If the existing implementation already computes the requested result, make the smallest legitimate change needed to satisfy the remaining mutation requirements rather than emitting a no-op.
CRITICAL RULE 0: NATIVE TOOLS ONLY (read_file, read_function, read_file_chunk, execute_command, patch_function, write_file, replace_lines, diff_patch, search_code, search_memory, run_tests, synthesize_report, halt).
CRITICAL RULE 1: If the EVIDENCE contains an exact file path outputted by a previous command, you MUST use that exact path.
CRITICAL RULE 2: If EVIDENCE satisfies the strategy, declare HALT.
CRITICAL RULE 3: There is NO downstream translator. Your proposal is parsed directly for a JSON action array -- natural-language-only descriptions of intent will NOT be executed and will waste a turn. You MUST end your response with a valid JSON array of action object(s) matching the ALLOWED ACTIONS schema (e.g. [{{"type": "read_file", "path": "...", "reasoning": "..."}}]). This applies to ALL actions, not just write_file/replace_lines/diff_patch.
CRITICAL RULE 4: NATIVE WRITE PROTOCOL. For write_file, use ONLY the native
write_file function supplied in the tool schema. Its arguments are exactly:
"path", "content", and "reasoning". The "content" field MUST contain only the
actual requested file content. Do NOT include system prompts, protocol
instructions, planning, hidden reasoning, JSON-format instructions, or
tool-use discussion in the content.

CRITICAL RULE 5: SYNTHESIZE_REPORT USAGE. If you are instructed to generate,
synthesize, or output a final report based on gathered evidence, use the native
write_file tool directly. The write_file "content" field must contain only the
final requested report content.

CRITICAL RULE 6: [ATHENA OMNISCIENCE] You are aware of several external compiled
sub-agents (WORKSPACE TOOL CONTRACTS like impact_analyzer, benchmark_runner,
mutation_ranker). To use them, you MUST use the execute_command native tool,
passing the binary path and arguments as the command string.

Do not expose or reproduce your internal reasoning or tool-protocol
deliberation in any action argument. For write_file, the "content" argument
must contain only the requested artifact. The native tool call is the
authoritative action representation."#,
            mission, strategy, world_context, bounded_evidence
        );

        let strategy_tools = crate::tools::schema::tool_declarations_for_strategy(phase, strategy);

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
        if let Some(expected_tool) = crate::tools::schema::native_tool_name_for_strategy(strategy) {
            payload["forced_tool_choice"] = serde_json::json!({
                "type": "function",
                "function": {
                    "name": expected_tool
                }
            });
        }

        let models = match nexus_algorithms::Router::select("action_planning") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };
        Self::safe_api_call(client, api_key, &models, &payload).await
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
the XML tags above. Do NOT veto legitimate search/recon commands. When the mission directive or context contains the necessary analysis details, writing the requested report or artifact is fully authorized without requiring codebase searches."#,
            proposed_action
        );
        let proposed_action = &p_armored;

        let filtered_history = Self::critic_visible_history(evidence_history);
        // Judgment-layer window: recent material plus every physically read
        // source region, so RULE 6 can pair claims with quotes.
        let grounded_evidence = Self::compress_preserving_reads(&filtered_history, 1500, 2500);

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
target file or a success marker does not exist yet.
CRITICAL RULE 3 [PATCH MUTATION SEMANTICS]: For a patch_function proposal, evaluate
the ACTUAL proposed `new_body`, not merely its computational equivalence to the
previous implementation. A patch is NOT a no-op when it makes an explicit,
mission-required source change such as adding documentation/comments, improving
clarity, cleanup, or another requested implementation detail, even if the
runtime result remains identical. If the ORIGINAL DIRECTIVE requires a
"documented implementation" and the proposed `new_body` adds appropriate
documentation or comments while preserving the requested behavior, this is a
legitimate mutation and MUST NOT receive a no-op veto. Only call a patch a
no-op when the proposed implementation is materially identical to the existing
implementation and does not satisfy any remaining mutation requirement.
CRITICAL RULE 4: CONSTRUCTIVE VETO.
CRITICAL RULE 5 [ANTI-REDUNDANT-READ]: If EVIDENCE HISTORY already contains a successful [READ_OK] for a file, you MUST VETO any subsequent read_file on that exact same path with a negative modifier (-0.30) and instruct the Strategist to proceed immediately to writing or patching.
CRITICAL RULE 5.5 [NEGATIVE RESULTS ARE VALID EVIDENCE]: A search_code, read_file, execute_command, or other inspection action that finds nothing (e.g. "No code matches found", a file that does not exist, an empty directory) is NOT a failed or invalid action -- it is successful evidence gathering that has physically confirmed an absence. Do NOT veto an inspection action merely because its result was negative. This is especially true when the mission or strategy is explicitly about checking whether something exists, is present, or has already been created: a "not found" result directly and correctly answers that question. Only veto an inspection action for reasons unrelated to the polarity of its result (e.g. wrong path entirely, malformed query, redundant with a prior identical attempt). If you issue a negative confidence_modifier, your critique
MUST explicitly state the exact command or action the Strategist should take
next to escape the failure. CRITICAL RULE 3: THE EVIDENCE VERIFICATION TAXONOMY.
If the proposed action contains a quantitative claim, architectural assumption,
or performance metric, you MUST check the EVIDENCE HISTORY. If there is no
explicit [Measured] execution log proving the claim, you MUST VETO the proposal.
CRITICAL RULE 6 [CLAIM CHECK]: When the PROPOSED ACTION is a halt on an analysis or audit objective, verify every claimed behavior against the actual source text in EVIDENCE HISTORY before approving. Quote the exact source lines that support or contradict each numbered claim in the critique. If a claim cannot be paired with source lines, or if the source shows different behavior than claimed, VETO with a negative confidence_modifier and name the contradicted claim.
CRITICAL RULE 7 [NO-DEFECT HALTS]: When the PROPOSED ACTION is a halt that reports NO defect on an audit whose directive says a defect may be reported only if the source establishes a violated contract, you may veto it ONLY by supplying a concrete counterexample: specific argument values for the function, and the exact sequence of branches the code takes for them, ending in the incorrect result. Walk the early returns and guards in order, since an earlier return can make a later branch unreachable for those inputs. A code comment is not a contract and cannot by itself establish a violation. If you cannot supply such a counterexample, approve the halt.
OUTPUT ONLY VALID JSON: {{"critique": "", "confidence_modifier": <float from
-0.5 to 0.5>}}"#,
            preamble = CRITIC_GROUNDING_PREAMBLE,
            mission = mission,
            strategy = strategy,
            proposed_action = proposed_action,
            world_context = world_context,
            grounded_evidence = grounded_evidence
        );

        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.0, "maxOutputTokens": 4096 }});
        let models_default = match nexus_algorithms::Router::select("critique") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };
        let models: Vec<String> = match std::env::var("NEXUS_CRITIC_MODEL") {
            Ok(v) if !v.trim().is_empty() => {
                v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
            }
            _ => models_default,
        };
        let text = Self::safe_api_call(client, api_key, &models, &payload).await?;
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

        let models = match nexus_algorithms::Router::select("action_translation") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };

        Self::safe_api_call(client, api_key, &models, &payload).await
    }

    pub async fn evaluate_evidence(
        client: &Client,
        api_key: &str,
        strategy: &str,
        action: &str,
        evidence: &str,
    ) -> Result<CriticResponse> {
        let truncated_evidence: String = Self::compress_preserving_reads(evidence, 1000, 1500);
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

        let models = match nexus_algorithms::Router::select("evaluate") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };
        let text = Self::safe_api_call(client, api_key, &models, &payload).await?;
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
        let truncated_evidence: String = Self::compress_preserving_reads(evidence, 3000, 11000);
        // The halt text IS the claim source for read-only analysis halts; the
        // claim-check below must see the full claims. The old 2000-char cap
        // silently truncated ROOT_CAUSE/TRACE/FIX answers mid-claim, leaving
        // the verifier to judge only their openings.
        let truncated_action: String = action.chars().take(6000).collect();
        let prompt = format!(
            r#"You are the Epistemic Verifier.
STRATEGY: {}
ACTION TAKEN: {}
EVIDENCE:
{}
Does this evidence mathematically prove the stated strategy's objective was achieved? A successful action (like [WRITE_OK] or [CMD_OK]) is NOT proof of success on its own UNLESS the ACTION TAKEN itself contains the actual content that was written or patched (write_file/diff_patch/patch_function actions above include the real file content directly in ACTION TAKEN) — in that case, check whether that content satisfies the objective, and treat a confirmed match as proof. Do not reject a successful write or patch merely because the bare EVIDENCE line (e.g. [WRITE_OK]) doesn't repeat the content; the content to check is in ACTION TAKEN.
For a [HALT] action on a read-only analysis or audit objective, the halt text IS the deliverable. Accept it only if (1) it states the concrete findings the objective asks for, not merely that the analysis was completed, and (2) read markers ([READ_OK] or [READ_CHUNK_OK]) for the files it discusses appear anywhere in EVIDENCE, including the historical section. Reject a halt that only claims the work is done. Also check every claimed behavior against the actual source text in EVIDENCE: reject the halt if any claim contradicts the code (for example, saying a branch returns X when the source shows it returns Y), or if a claimed defect is documented as intentional in the code's own comments, and name the problem in the rationale. Before deciding, quote the exact source lines from EVIDENCE that support or contradict each numbered claim; any claim that cannot be paired with source lines is unverified and must fail.
Reply with a single JSON object and nothing else. The first character of your reply must be an opening curly brace.
OUTPUT ONLY VALID JSON matching this schema:
{{"rationale": "<reasoning and exact source quotes>", "goal_satisfied": true, "evidence_confidence": 0.9, "independent_check_passed": true}}"#,
            strategy, truncated_action, truncated_evidence
        );

        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.0, "maxOutputTokens": 1024 }});
        // Verification is the last gate before mission authorization: serve
        // it from the heavy tier. A lightweight verifier has been observed
        // rubber-stamping halt text that contradicts the source in evidence,
        // and the NVIDIA fallback tier is unreachable from this host, which
        // silently drops the cascade to its weakest member.
        // To restore Router routing, replace the line below with the former
        // `match nexus_algorithms::Router::select("verify") { ... }` block.
        let models = crate::model_config::get_heavy_models();
        eprintln!("[VERIFIER] model cascade: {:?}", models);
        let mut text = Self::safe_api_call(client, api_key, &models, &payload).await?;
        if serde_json::from_str::<serde_json::Value>(extract_json_block(&sanitize_json_escapes(
            &text,
        )))
        .is_err()
        {
            eprintln!("[VERIFIER] non-JSON reply; retrying once");
            text = Self::safe_api_call(client, api_key, &models, &payload).await?;
        }
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
        let truncated_evidence: String = Self::compress_preserving_reads(evidence, 1000, 1500);
        let prompt = format!(
            r#"You are the Meta-Cognitive Reflector.
STRATEGY: {}
EVIDENCE (Failure/Anomaly):
{}
Analyze why the strategy failed to achieve the objective based on the evidence. Provide a concise 2-sentence reflection on the error and suggest a pivot. OUTPUT ONLY PLAIN TEXT."#,
            strategy, truncated_evidence
        );
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.3, "maxOutputTokens": 1024 }});
        let models = match nexus_algorithms::Router::select("reflect") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };
        Self::safe_api_call(client, api_key, &models, &payload).await
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

#[cfg(test)]
mod tests {
    use super::EpistemicEngine;
    use reqwest::Client;

    #[tokio::test]
    async fn compound_patch_mission_prefers_patch_function_over_verification_command() {
        let directive = r#"Patch `apps/math_utils/src/lib.rs` function `add` so that it returns exactly `a + b`.

You MUST verify the mutation by running `cargo check -p math_utils`."#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("compound patch mission should produce a deterministic strategy");

        assert_eq!(
            strategies,
            vec![
                "patch_function:apps/math_utils/src/lib.rs:add".to_string(),
                "read_file:apps/math_utils/src/lib.rs".to_string(),
                "replace_lines:apps/math_utils/src/lib.rs".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn command_only_mission_still_prefers_execute_command() {
        let directive = "Execute cargo check -p math_utils to verify the package.";

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("command-only mission should produce a deterministic strategy");

        assert_eq!(strategies, vec!["execute_command:cargo check -p math_utils".to_string()]);
    }

    #[tokio::test]
    async fn explicit_read_ok_criterion_binds_to_named_file() {
        let directive = r#"OBJECTIVE: Inspect crates/runtime_core/src/cortical_loop.rs
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
READ_OK:crates/runtime_core/src/cortical_loop.rs

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced before this node can be considered complete."#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("READ_OK directive should produce a deterministic strategy");

        assert_eq!(
            strategies,
            vec!["read_file:crates/runtime_core/src/cortical_loop.rs".to_string()]
        );
    }

    #[tokio::test]
    async fn read_function_ok_still_wins_over_read_ok() {
        let directive = r#"OBJECTIVE: Inspect the route function
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
READ_OK:crates/runtime_core/src/router.rs
READ_FUNCTION_OK:crates/runtime_core/src/router.rs:route"#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("compound read directive should produce a deterministic strategy");

        assert_eq!(
            strategies,
            vec!["read_function:crates/runtime_core/src/router.rs:route".to_string()]
        );
    }

    #[tokio::test]
    async fn explicit_search_memory_directive_is_authoritative() {
        let directive = "Use the SearchMemory tool to recall prior memory specifically about host observations involving uptime, filesystem or disk capacity, available memory/RAM, and load average.";

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("explicit SearchMemory directive should produce a deterministic strategy");

        assert_eq!(strategies.len(), 1);
        assert_eq!(strategies[0], format!("search_memory:{}", directive));
    }

    #[tokio::test]
    async fn compound_read_plus_memory_pushes_search_memory_first() {
        let directive = r#"OBJECTIVE: Inspect the compiler-critic failure branch.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
READ_OK:crates/runtime_core/src/cortical_loop.rs
SEARCH_MEMORY_OK:compiler_critic_prior_lessons

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced."#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("compound READ + MEMORY mission should produce strategies");

        assert_eq!(
            strategies[0], "search_memory:compiler_critic_prior_lessons",
            "search_memory must be selected before read_file; got {:?}",
            strategies
        );
        assert!(
            strategies.iter().any(|s| s == "read_file:crates/runtime_core/src/cortical_loop.rs"),
            "read_file must still be present as a fallback"
        );
    }

    // Source-inspection guard
    // The verifier's claim-check depends on BOTH the untruncated halt text
    // and the grounding instruction; losing either silently reverts the
    // verifier to rubber-stamping.
    #[test]
    fn verifier_claim_check_needs_full_claims_and_grounded_quotes() {
        let source = include_str!("epistemic_engine.rs");
        assert!(
            source.contains("action.chars().take(6000)"),
            "halt claims must not be truncated at 6000 chars"
        );
        assert!(
            source.contains("quote the exact source lines from EVIDENCE"),
            "claim-check must require grounded quotes before the verdict"
        );
        assert!(
            source.contains("eprintln!(\"[VERIFIER] model cascade"),
            "verify model cascade must be logged"
        );
    }

    #[tokio::test]
    async fn combined_mission_root_context_cannot_contaminate_node_strategy() {
        // This reproduces the observed OpenFang failure:
        // the current node must execute git clone, while a later mission step
        // in ROOT DIRECTIVE CONTEXT asks for write_file. Deterministic strategy
        // selection must ignore that background context.
        let directive = r#"OBJECTIVE: Execute git clone https://github.com/RightNow-AI/openfang.git /apps/openfang.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
CMD_OK:git clone https://github.com/RightNow-AI/openfang.git /apps/openfang

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced before this node can be considered complete. Cortex belief alone is never sufficient. Node completion requires the exact command evidence.

ROOT DIRECTIVE CONTEXT:
MISSION UPDATE (ARCHITECT MODE):
Clone the repository, analyze its architecture, compare its memory and WASM sandboxing, and use write_file to generate ROADMAP_OPENFANG_COMPARISON.md.
Later step: use write_file:ROADMAP_OPENFANG_COMPARISON.md.

LINEAGE CONTEXT:
A later analysis node will inspect files and synthesize a roadmap using write_file."#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("combined mission should produce a deterministic command strategy");

        assert_eq!(
            strategies,
            vec![
                "execute_command:git clone https://github.com/RightNow-AI/openfang.git /apps/openfang"
                    .to_string()
            ],
            "root/lineage write instructions must not contaminate the node-local command strategy"
        );
    }

    #[test]
    fn critic_prompt_carries_grounded_claim_check() {
        let source = include_str!("epistemic_engine.rs");
        assert!(
            source.contains("CRITICAL RULE 6 [CLAIM CHECK]"),
            "critic must verify halt claims against source before approving"
        );
    }

    #[test]
    fn safe_api_call_logs_every_attempt() {
        let source = include_str!("epistemic_engine.rs");
        assert!(
            source.contains("[MODEL TRACE] -> attempt model="),
            "each cascade attempt must be logged for serving-model attribution"
        );
    }

    #[test]
    fn compress_preserving_reads_keeps_whole_file_content() {
        let body: String =
            (0..200).map(|i| format!("fn line_{}() {{}}", i)).collect::<Vec<_>>().join("\n");
        let read = format!("[READ_OK] x.rs\nFULL FILE CONTENT:\n{}\n", body);
        let noise: String = (0..120).map(|i| format!("noise {}", i)).collect::<Vec<_>>().join("\n");
        let evidence =
            format!("line_a\nline_b\n{}\n[PROPOSITION] end-of-read\n{}\ntail_marker", read, noise);

        let out = EpistemicEngine::compress_preserving_reads(&evidence, 64, 64);
        assert!(out.contains("FULL FILE CONTENT:"), "read record must survive");
        assert!(out.contains("fn line_199() {}"), "end of the file body must survive");
        assert!(out.contains("line_a"), "head must survive");
        assert!(out.contains("tail_marker"), "tail must survive");
        assert!(out.contains("[... MIDDLE EVIDENCE COMPRESSED ...]"), "elision must be marked");
        assert!(!out.contains("noise 100"), "middle non-read noise must be elided");
    }

    #[test]
    fn compress_preserving_reads_keeps_short_evidence_verbatim() {
        let evidence = "[READ_OK] x.rs\nFULL FILE CONTENT:\nfn main() {}";
        let out = EpistemicEngine::compress_preserving_reads(evidence, 64, 64);
        assert_eq!(out, evidence);
    }

    #[test]
    fn compress_preserving_reads_caps_oversized_reads_visibly() {
        let body: String =
            (0..2000).map(|i| format!("fn line_{}() {{}}", i)).collect::<Vec<_>>().join("\n");
        let read = format!("[READ_OK] big.rs\nFULL FILE CONTENT:\n{}\n", body);
        let out = EpistemicEngine::compress_preserving_reads(&read, 64, 64);
        assert!(
            out.contains("[... READ BODY CAPPED AT 12000 CHARS IN JUDGE WINDOW ...]"),
            "oversized reads must be capped with a visible marker"
        );
        assert!(out.contains("fn line_0() {}"), "the head of the read must survive");
    }

    #[test]
    fn bracketed_source_lines_do_not_end_a_pinned_read() {
        let read = "[READ_OK] s.rs\nFULL FILE CONTENT:\n[ERROR] marker-shaped source line\nfn after() {}\n";
        let out = EpistemicEngine::compress_preserving_reads(read, 8, 8);
        assert!(
            out.contains("fn after() {}"),
            "unknown bracketed lines must not terminate the pinned read"
        );
    }

    #[test]
    fn critic_visible_history_drops_echo_chunks_and_keeps_reads() {
        let read = "[READ_OK] x.rs\nFULL FILE CONTENT:\nfn target() {}\n";
        let history = format!(
            "{}\n\n[CRITIC_VETO]\nold verdict text\n\n[HALT_REJECTED]\nold rejection text\n\ntail_line",
            read
        );
        let visible = EpistemicEngine::critic_visible_history(&history);
        assert!(!visible.contains("old verdict text"), "echo veto must be filtered");
        assert!(!visible.contains("old rejection text"), "old halt rejections must be filtered");
        assert!(visible.contains("fn target() {}"), "physically read source must remain visible");
        let windowed = EpistemicEngine::compress_preserving_reads(&visible, 64, 64);
        assert!(windowed.contains("fn target() {}"), "judge window must keep the read whole");
    }

    #[tokio::test]
    async fn compound_read_plus_search_code_pushes_search_first() {
        let directive = r#"OBJECTIVE: Inspect the compiler-critic failure branch.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
READ_OK:crates/runtime_core/src/cortical_loop.rs
SEARCH_CODE_OK:compiler_critic

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced."#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("compound READ + SEARCH_CODE mission should produce strategies");

        assert!(
            strategies[0].starts_with("search_code:"),
            "search_code must be selected before read_file; got {:?}",
            strategies
        );
        assert!(
            strategies.iter().any(|s| s == "read_file:crates/runtime_core/src/cortical_loop.rs"),
            "read_file must still be present as a fallback"
        );
    }
}
