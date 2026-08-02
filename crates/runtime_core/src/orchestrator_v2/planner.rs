use crate::orchestrator_v2::task_graph::{TaskGraph, TaskNode, TaskStatus};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

pub struct PlannerAgent;

impl PlannerAgent {
    pub async fn generate_plan(
        client: &Client,
        api_key: &str,
        directive: &str,
    ) -> Result<TaskGraph> {
        let system_instruction =
            "You are a headless JSON API. You MUST output ONLY a valid, raw JSON array. \
            Do NOT use markdown formatting like ```json. Do NOT write any conversational text. \
            Do NOT explain your steps. Your output must start exactly with `[` and end with `]`. \
            CRITICAL RULE 1: POLYGLOT TOOLING MATRIX. Assign correct CLI tools based on language. For Rust: `cargo clippy --workspace` & `cargo bench`. For Python: `python3 -m py_compile` & `python3 -m timeit`. For JS/TS: `node -c` & `npm run lint`. STILL use symbol_graph to mathematically resolve ALL paths! CRITICAL RULE: NEVER propose or execute `cargo clean` or `make clean`. Rely on incremental compilation! \
            CRITICAL RULE 2 [AST-FIRST]: You are in a Zero Fragmentation multi-crate workspace. NEVER guess file paths. Before invoking read_file, you MUST query the symbol_graph to resolve the exact absolute workspace path. Do not use raw OS shell commands like find.
            CRITICAL RULE 2: You MUST explicitly chain your tasks using the 'deps' array so they execute in a strict, linear sequence. Task 2 must depend on Task 1, Task 3 must depend on Task 2, etc. DO NOT leave the 'deps' array empty unless it is the very first task. \
            CRITICAL RULE 3: The ONLY valid fields for each object are 'id', 'objective', 'deps', 'required_agent', and 'success_criteria'. \
            CRITICAL RULE 4: THE EVIDENCE VERIFICATION TAXONOMY. If a task requires generating a report, analysis, or quantitative claim, you MUST embed instructions in the 'objective' requiring the use of epistemic tags: [Observed], [Derived], [Measured], [Hypothesis]. \
            DO NOT output any other fields like 'params', 'required_files', or 'Architectural_Strategies'. Embed all details into the 'objective' string. \
            ANTI-ANCHORING RULE: The example below exists solely to show you the required JSON structure. Your plan's tasks must be derived entirely from the REAL DIRECTIVE.";

        let payload = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": system_instruction }]
            },
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": "FORMAT EXAMPLE ONLY:\nDIRECTIVE: Add a one-line comment to README.md." }]
                },
                {
                    "role": "model",
                    "parts": [{ "text": "[\n  {\n    \"id\": \"task_1\",\n    \"objective\": \"Add comment to README.md\",\n    \"deps\": [],\n    \"required_agent\": \"GeneralAgent\",\n    \"success_criteria\": [\"Edit completes\"]\n  }\n]" }]
                },
                {
                    "role": "user",
                    "parts": [{ "text": format!("REAL DIRECTIVE: {}", directive) }]
                },
                {
                    "role": "model",
                    "parts": [{ "text": "[" }] // ⚡ FORCE PRE-FILL
                }
            ],
            "generationConfig": {
                "temperature": 0.1,
                "topK": 40,
                "topP": 0.95,
                "maxOutputTokens": 8192
            }
        });

        let base_url = std::env::var("NEXUS_API_GATEWAY")
            .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string());

        let url = format!(
            "{}/v1beta/models/gemini-3.1-flash-lite:generateContent?key={}",
            base_url, api_key
        );

        let mut attempts = 0;
        let mut last_error = String::new();

        while attempts < 6 {
            attempts += 1;
            match client.post(&url).json(&payload).send().await {
                Ok(res) => {
                    let status = res.status();
                    let body: Value = res.json().await.unwrap_or_default();

                    if status == 429 || status.as_u16() == 503 {
                        let wait = attempts * 5;
                        tracing::warn!(
                            "[PLANNER] API Throttled ({}). Retrying in {}s...",
                            status,
                            wait
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                        continue;
                    }

                    if let Some(err) = body.pointer("/error/message").and_then(|s| s.as_str()) {
                        if err.contains("Internal error")
                            || err.contains("high demand")
                            || err.contains("quota")
                            || err.contains("rate limit")
                        {
                            tracing::warn!("[PLANNER] Transient fault: {}. Retrying...", err);
                            tokio::time::sleep(std::time::Duration::from_secs(attempts * 5)).await;
                            continue;
                        }
                        return Err(anyhow!("API Error: {}", err));
                    }

                    let mut text = body
                        .pointer("/candidates/0/content/parts/0/text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if text.trim().is_empty() {
                        tracing::warn!("[PLANNER] Empty response received. Retrying...");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }

                    if !text.starts_with('[') {
                        text = format!("[{}", text);
                    }

                    let array_str = match Self::extract_json_array(&text) {
                        Some(s) => s,
                        None => {
                            tracing::warn!("[PLANNER] JSON format fracture. Retrying...");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                    };

                    match serde_json::from_str::<Vec<TaskNode>>(&array_str) {
                        Ok(nodes) => {
                            if nodes.is_empty() {
                                continue;
                            }
                            let mut graph = TaskGraph::new();
                            for mut node in nodes {
                                node.status = TaskStatus::Pending;
                                graph.nodes.insert(node.id.clone(), node);
                            }
                            return Ok(graph);
                        }
                        Err(e) => {
                            tracing::warn!("[PLANNER] DAG Parse Fracture: {}. Retrying...", e);
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                }
            }
        }
        Err(anyhow!("FATAL TRANSPORT FRACTURE after 6 attempts. Last error: {}", last_error))
    }

    fn extract_json_array(text: &str) -> Option<String> {
        let start = text.find('[')?;
        let mut depth = 0;
        for (i, c) in text[start..].char_indices() {
            match c {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(text[start..=start + i].to_string());
                    }
                }
                _ => {}
            }
        }
        let partial = &text[start..];
        let mut end_idx = partial.len();
        while let Some(last_close) = partial[..end_idx].rfind('}') {
            let candidate = format!("{}]", &partial[..=last_close]);
            let clean_candidate: String =
                candidate.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').collect();
            if serde_json::from_str::<serde_json::Value>(&clean_candidate).is_ok() {
                return Some(clean_candidate);
            }
            end_idx = last_close;
        }
        None
    }
}
