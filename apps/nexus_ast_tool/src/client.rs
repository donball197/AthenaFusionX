use anyhow::{anyhow, Result};
use serde_json::json;
use std::env;
use std::thread;
use std::time::Duration;

pub struct ApiClient;

struct LlmEndpoint {
    is_gemini: bool,
    model: String,
    url: String,
    api_key: String,
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiClient {
    pub fn new() -> Self {
        Self
    }

    fn resolve_endpoint() -> Result<LlmEndpoint> {
        let model = Self::resolve_model()?;
        let lower = model.to_lowercase();
        if lower.contains("gemini") || lower.contains("gemma") {
            Ok(LlmEndpoint {
                is_gemini: true,
                model: model.clone(),
                url: format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
                ),
                api_key: env::var("GEMINI_API_KEY")
                    .map_err(|_| anyhow!("GEMINI_API_KEY required"))?,
            })
        } else {
            Ok(LlmEndpoint {
                is_gemini: false,
                model,
                url: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
                api_key: env::var("NVIDIA_API_KEY")
                    .map_err(|_| anyhow!("NVIDIA_API_KEY required"))?,
            })
        }
    }

    fn call(prompt: &str, temperature: f32) -> Result<String> {
        let ep = Self::resolve_endpoint()?;
        let body = if ep.is_gemini {
            json!({
                "contents": [{ "parts": [{ "text": prompt }] }],
                "generationConfig": { "temperature": temperature }
            })
        } else {
            json!({
                "model": ep.model,
                "messages": [{ "role": "user", "content": prompt }],
                "temperature": temperature
            })
        };

        let mut req = ureq::post(&ep.url)
            .timeout(Duration::from_secs(120))
            .set("Content-Type", "application/json");
        req = if ep.is_gemini {
            req.set("x-goog-api-key", &ep.api_key)
        } else {
            req.set("Authorization", &format!("Bearer {}", ep.api_key))
        };

        Self::extract_text(req.send_json(body)?, ep.is_gemini)
    }

    fn with_retry_transient<T, F: FnMut() -> Result<T>>(mut f: F, max_attempts: u32) -> Result<T> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match f() {
                Ok(v) => return Ok(v),
                Err(e) => {
                    let transient = match e.downcast_ref::<ureq::Error>() {
                        Some(ureq::Error::Status(429, _)) => true,
                        Some(ureq::Error::Status(code, _)) => (500u16..600u16).contains(code),
                        Some(ureq::Error::Transport(_)) => true,
                        None => false,
                    };
                    if !transient || attempt >= max_attempts {
                        return Err(e);
                    }
                    let backoff_ms = 1000u64 * 2u64.pow(attempt - 1);
                    eprintln!(
                        "        [🔄 Retry] attempt {} failed (transient): {}. Backing off {}ms...",
                        attempt, e, backoff_ms
                    );
                    thread::sleep(Duration::from_millis(backoff_ms));
                }
            }
        }
    }

    fn sanitize_json(raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.peek() {
                    Some('"') | Some('\\') | Some('/') | Some('n') | Some('t')
                    | Some('r') | Some('b') | Some('f') | Some('u') => out.push(c),
                    _ => {
                        out.push('\\');
                        out.push('\\');
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    fn strip_code_fences(raw: &str) -> String {
        let mut t = raw.trim();
        if let Some(rest) = t.strip_prefix("```") {
            let rest = rest.split_once('\n').map(|(_, r)| r).unwrap_or(rest);
            t = rest.trim_end();
            if let Some(body) = t.strip_suffix("```") {
                t = body;
            }
        }
        t.trim().to_string()
    }

    fn extract_payload(text: &str) -> Result<String> {
        let start = text
            .find("<payload>")
            .ok_or_else(|| anyhow!("Missing <payload> in model output"))?
            + "<payload>".len();
        let end = text
            .rfind("</payload>")
            .ok_or_else(|| anyhow!("Missing </payload> in model output"))?;
        if end < start {
            return Err(anyhow!("Malformed <payload> block"));
        }
        Ok(Self::strip_code_fences(&text[start..end]))
    }

    pub fn triage_vulnerability(finding_type: &str, code_snippet: &str) -> Result<(String, String, String)> {
        Self::with_retry_transient(|| Self::triage_vulnerability_attempt(finding_type, code_snippet), 3)
    }

    fn triage_vulnerability_attempt(finding_type: &str, code_snippet: &str) -> Result<(String, String, String)> {
        thread::sleep(Duration::from_millis(2000));
        let prompt = format!(
            "You are an elite Security Triage AI.\n\
             Analyze this code for a '{}' vulnerability.\n\
             Source:\n\
             ```\n{}\n```\n\
             CRITICAL DIRECTIVES:\n\
             1. If definitely vulnerable and untrusted data reaches the sink, return \"EXPLOITABLE\".\n\
             2. If definitely mitigated (e.g. prepared statement, integer cast), return \"SAFE\".\n\
             3. If you must make ANY ASSUMPTION about external data validation, session trust, or provenance, return \"UNKNOWN\".\n\
             4. \"SAFE\" is permitted ONLY when the supplied source proves the sink is mitigated without assumptions.\n\
             5. The state MUST be exactly one of: \"EXPLOITABLE\", \"SAFE\", \"UNKNOWN\".\n\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:\n\
             {{\"state\": \"EXPLOITABLE\", \"severity\": \"High\", \"reason\": \"...\"}}",
            finding_type, code_snippet
        );

        let text = Self::call(&prompt, 0.1)?;
        let start = text.find("<payload>").map(|i| i + "<payload>".len())
            .unwrap_or_else(|| text.find('{').unwrap_or(0));
        let end = text.rfind("</payload>")
            .unwrap_or_else(|| text.rfind('}').map(|i| i + 1).unwrap_or(text.len()))
            .max(start);
        let json_str = Self::strip_code_fences(&text[start..end]);
        let sanitized = Self::sanitize_json(&json_str);

        let parsed: serde_json::Value =
            serde_json::from_str(&sanitized).map_err(|e| anyhow!("JSON Parse Error: {}", e))?;
        let state = match parsed["state"].as_str().unwrap_or("").trim().to_uppercase().as_str() {
            "EXPLOITABLE" => "EXPLOITABLE",
            "SAFE" => "SAFE",
            "UNKNOWN" => "UNKNOWN",
            _ => "UNKNOWN",
        };

        Ok((
            state.to_string(),
            parsed["severity"].as_str().unwrap_or("Unknown").to_string(),
            parsed["reason"].as_str().unwrap_or("None").to_string(),
        ))
    }

    pub fn generate_security_patch(finding_type: &str, code_snippet: &str, lang: &str) -> Result<String> {
        Self::with_retry_transient(
            || Self::generate_security_patch_attempt(finding_type, code_snippet, lang),
            3,
        )
    }

    fn generate_security_patch_attempt(finding_type: &str, code_snippet: &str, lang: &str) -> Result<String> {
        thread::sleep(Duration::from_millis(2000));
        let prompt = format!(
            "You are an elite Security Auto-Healer.\n\
             Fix this '{}' vulnerability in this {} code.\n\
             The code below occupies whole lines and will be REPLACED EXACTLY as a block:\n\
             ```\n\
             {}\n\
             ```\n\
             Return ONLY the replacement code, wrapped EXACTLY in <payload></payload> tags.\n\
             No markdown fences, no explanations.\n\
             Constraints:\n\
             1. For SQL injection: use prepared statements / parameterized queries. Do NOT merely escape strings - escaping-only patches are rejected.\n\
             2. In PHP, mysqli_stmt_bind_param takes arguments BY REFERENCE: every bound argument must be a plain variable. Hoist expressions like md5($x) into a variable first.\n\
             3. The vulnerable statement(s) must be REMOVED, not kept beside the fix.\n\
             4. Match the snippet's indentation.",
            finding_type, lang, code_snippet
        );

        let text = Self::call(&prompt, 0.1)?;
        Self::extract_payload(&text)
    }

    pub fn generate_optimization(symbol: &str, code_snippet: &str) -> Result<String> {
        let prompt = format!(
            "You are the Nexus Singularity Architect. Optimize this Rust function '{}' for better performance, readability, or idiomatic Rust.\n\nSource:\n```rust\n{}\n```\n\nReturn ONLY the optimized Rust statements for the replacement function body wrapped EXACTLY in <payload> and </payload> tags. Do not include outer braces.",
            symbol, code_snippet
        );
        let text = Self::with_retry_transient(|| Self::call(&prompt, 0.1), 3)?;
        Self::extract_payload(&text)
    }

    pub fn query_model(instruction: &str, symbol: &str, code_snippet: &str, strategy: &str) -> Result<String> {
        Self::query_model_internal(instruction, symbol, code_snippet, strategy, None)
    }

    pub fn query_model_retry(
        instruction: &str,
        symbol: &str,
        code_snippet: &str,
        strategy: &str,
        validation_error: &str,
        previous_payload: &str,
    ) -> Result<String> {
        Self::query_model_internal(
            instruction,
            symbol,
            code_snippet,
            strategy,
            Some((validation_error, previous_payload)),
        )
    }

    fn query_model_internal(
        instruction: &str,
        symbol: &str,
        code_snippet: &str,
        strategy: &str,
        correction: Option<(&str, &str)>,
    ) -> Result<String> {
        Self::with_retry_transient(
            || Self::query_model_attempt(instruction, symbol, code_snippet, strategy, correction),
            3,
        )
    }

    fn query_model_attempt(
        instruction: &str,
        symbol: &str,
        code_snippet: &str,
        strategy: &str,
        correction: Option<(&str, &str)>,
    ) -> Result<String> {
        thread::sleep(Duration::from_millis(2000));

        let strategy_contract = match strategy {
            "RenameSymbol" => "Payload must be a single valid Rust identifier (the new name). No quotes, no keywords, no punctuation.",
            "ReplaceFunctionBody" => "Payload must be the exact statements that will replace the function body. Do NOT include the outer braces, the fn signature, or any surrounding code.",
            "MakePublic" => "Payload must be empty (the strategy ignores it). Emit nothing between the tags.",
            "InsertEarlyReturn" => "Payload must be a valid Rust expression that will be returned (e.g. `()` or `None` or `Err(...)`). Do not include the `return` keyword or the trailing semicolon.",
            "InsertComment" => "Payload must be the comment text only (no // or /* */ markers).",
            _ => "Payload must satisfy the contract of the named strategy. Prefer the minimal correct value.",
        };

        let mut prompt = format!(
            "You are the Nexus Mutation Payload Generator.\n\
             The mutation strategy is FIXED and must not be changed: \"{strategy}\"\n\n\
             Strategy contract:\n\
             {strategy_contract}\n\n\
             Natural-language instruction from the user:\n\
             {instruction}\n\n\
             Target symbol: {symbol}\n\n\
             Source context (for reference only):\n\
             ```rust\n\
             {code_snippet}\n\
             ```\n\n\
             CRITICAL RULES:\n\
             - Generate ONLY the payload required by the strategy.\n\
             - Do NOT emit the function signature, outer braces, markdown fences, or any explanation.\n\
             - Wrap the payload EXACTLY between <payload> and </payload> tags.\n\
             - If the strategy expects an empty payload, leave the tags empty: <payload></payload>\n",
            strategy = strategy,
            strategy_contract = strategy_contract,
            instruction = instruction,
            symbol = symbol,
            code_snippet = code_snippet,
        );

        if let Some((validation_error, previous_payload)) = correction {
            prompt.push_str(&format!(
                "\n\nCORRECTIVE RETRY (exactly one attempt is allowed):\n\
                 The previous payload was rejected by the strategy validator.\n\n\
                 Validator error:\n\
                 {validation_error}\n\n\
                 Previous (rejected) payload:\n\
                 {previous_payload}\n\n\
                 Produce a corrected payload that satisfies the strategy contract above. Still wrap it ONLY in <payload></payload> tags.\n",
                validation_error = validation_error,
                previous_payload = previous_payload,
            ));
        }

        let text = Self::call(&prompt, 0.1)?;
        let start = text.find("<payload>").map(|i| i + "<payload>".len()).unwrap_or(0);
        let end = text.rfind("</payload>").unwrap_or(text.len()).max(start);
        Ok(Self::strip_code_fences(&text[start..end]))
    }

    fn resolve_model() -> Result<String> {
        env::var("NEXUS_LIGHT_MODEL")
            .or_else(|_| env::var("NEXUS_MODEL"))
            .or_else(|_| {
                env::var("NEXUS_LIGHT_CASCADE")
                    .ok()
                    .and_then(|cascade| {
                        cascade.split(',').map(|s| s.trim().to_string()).find(|s| !s.is_empty())
                    })
                    .ok_or_else(|| {
                        anyhow!("No model specified. Set NEXUS_LIGHT_MODEL, NEXUS_MODEL, or NEXUS_LIGHT_CASCADE")
                    })
            })
    }

    fn extract_text(response: ureq::Response, is_gemini: bool) -> Result<String> {
        let json: serde_json::Value = response.into_json()?;
        if is_gemini {
            let mut full_text = String::new();
            if let Some(parts) = json["candidates"][0]["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(t) = part["text"].as_str() {
                        full_text.push_str(t);
                    }
                }
            }
            Ok(full_text)
        } else {
            Ok(json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
        }
    }
}
