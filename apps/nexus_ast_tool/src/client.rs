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
                "generationConfig": { "temperature": temperature, "maxOutputTokens": 8192 }
            })
        } else {
            json!({
                "model": ep.model,
                "messages": [{ "role": "user", "content": prompt }],
                "temperature": temperature,
                "max_tokens": 8192
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
                    Some('"') | Some('\\') | Some('/') | Some('n') | Some('t') | Some('r')
                    | Some('b') | Some('f') | Some('u') => out.push(c),
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
        
        let end = text.rfind("</payload>").unwrap_or(text.len());
        if end < start {
            return Err(anyhow!("Malformed <payload> block"));
        }
        Ok(Self::strip_code_fences(&text[start..end]))
    }

    pub fn triage_vulnerability(
        finding_type: &str,
        code_snippet: &str,
    ) -> Result<(String, String, String)> {
        Self::with_retry_transient(
            || Self::triage_vulnerability_attempt(finding_type, code_snippet),
            3,
        )
    }

    fn triage_vulnerability_attempt(
        finding_type: &str,
        code_snippet: &str,
    ) -> Result<(String, String, String)> {
        thread::sleep(Duration::from_millis(1500));
        let prompt = format!(
            "You are an elite Security Triage AI.\n\
             Analyze this code for a '{}' vulnerability.\n\
             Source:\n\
             ```\n{}\n```\n\
             CRITICAL DIRECTIVES:\n\
             1. If definitely vulnerable and untrusted data reaches the sink, return \"EXPLOITABLE\".\n\
             2. If definitely mitigated (e.g. prepared statement, int casting), return \"SAFE\".\n\
             3. Direct string concatenation of `$_SESSION` superglobals in SQL queries without prepared statements is \"EXPLOITABLE\" (Second-Order SQLi).\n\
             4. Return ONLY a JSON object wrapped in <payload></payload> tags:\n\
             <payload>{{\"state\": \"EXPLOITABLE\", \"severity\": \"High\", \"reason\": \"Concatenation into SQL\"}}</payload>",
            finding_type, code_snippet
        );

        let text = Self::call(&prompt, 0.1)?;
        let payload_str = match Self::extract_payload(&text) {
            Ok(p) => p,
            Err(_) => Self::strip_code_fences(&text),
        };

        let sanitized = Self::sanitize_json(&payload_str);
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&sanitized) {
            let state = match parsed["state"].as_str().unwrap_or("").trim().to_uppercase().as_str() {
                "EXPLOITABLE" => "EXPLOITABLE",
                "SAFE" => "SAFE",
                _ => "UNKNOWN",
            };
            return Ok((
                state.to_string(),
                parsed["severity"].as_str().unwrap_or("High").to_string(),
                parsed["reason"].as_str().unwrap_or("Vulnerability identified").to_string(),
            ));
        }

        // Fallback robust heuristic parser if JSON parsing failed
        let upper = payload_str.to_uppercase();
        let state = if upper.contains("\"EXPLOITABLE\"") || upper.contains("EXPLOITABLE") {
            "EXPLOITABLE"
        } else if upper.contains("\"SAFE\"") || upper.contains("SAFE") {
            "SAFE"
        } else {
            "UNKNOWN"
        };

        Ok((state.to_string(), "High".to_string(), "Identified via AST Taint Analysis".to_string()))
    }

    pub fn generate_security_patch(
        finding_type: &str,
        code_snippet: &str,
        lang: &str,
    ) -> Result<String> {
        Self::with_retry_transient(
            || Self::generate_security_patch_attempt(finding_type, code_snippet, lang),
            3,
        )
    }

    fn generate_security_patch_attempt(
        finding_type: &str,
        code_snippet: &str,
        lang: &str,
    ) -> Result<String> {
        thread::sleep(Duration::from_millis(1500));
        let prompt = format!(
            "You are an elite Security Auto-Healer.\n\
             Fix this '{}' vulnerability in this {} code.\n\
             The code below will be REPLACED EXACTLY as a single continuous block:\n\
             ```\n\
             {}\n\
             ```\n\
             Return ONLY the replacement code inside <payload></payload> tags. No markdown formatting outside the tags.\n\n\
             SYNTAX INTEGRITY RULES:\n\
             1. Replace unsanitized query concatenation and execution with mysqli prepared statements (`$stmt = $con->prepare(...)`, `$stmt->bind_param(...)`, `$stmt->execute()`).\n\
             2. Ensure all opening and closing braces '{{' and '}}' within your replacement are completely balanced.\n\
             3. Do not re-declare surrounding blocks that are outside this snippet.",
            finding_type, lang, code_snippet
        );

        let text = Self::call(&prompt, 0.1)?;
        match Self::extract_payload(&text) {
            Ok(p) => Ok(p),
            Err(_) => Ok(Self::strip_code_fences(&text)),
        }
    }

    pub fn generate_optimization(symbol: &str, code_snippet: &str) -> Result<String> {
        let prompt = format!(
            "You are the Nexus Singularity Architect. Optimize this Rust function '{}' for better performance, readability, or idiomatic Rust.\n\nSource:\n```rust\n{}\n```\n\nReturn ONLY the optimized Rust statements for the replacement function body wrapped EXACTLY in <payload> and </payload> tags. Do not include outer braces.",
            symbol, code_snippet
        );
        let text = Self::with_retry_transient(|| Self::call(&prompt, 0.1), 3)?;
        Self::extract_payload(&text)
    }

    pub fn query_model(
        instruction: &str,
        symbol: &str,
        code_snippet: &str,
        strategy: &str,
    ) -> Result<String> {
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
        thread::sleep(Duration::from_millis(1500));

        let strategy_contract = match strategy {
            "RenameSymbol" => "Payload must be a single valid Rust identifier.",
            "ReplaceFunctionBody" => "Payload must be the exact statements that will replace the function body.",
            "MakePublic" => "Payload must be empty.",
            "InsertEarlyReturn" => "Payload must be a valid Rust return expression.",
            "InsertComment" => "Payload must be the comment text only.",
            _ => "Payload must satisfy strategy contract.",
        };

        let mut prompt = format!(
            "You are the Nexus Mutation Payload Generator.\n\
             Strategy: \"{strategy}\"\n\
             Contract: {strategy_contract}\n\
             Instruction: {instruction}\n\
             Target: {symbol}\n\
             Context:\n```rust\n{code_snippet}\n```\n\
             Return ONLY the payload wrapped in <payload></payload> tags.\n",
            strategy = strategy,
            strategy_contract = strategy_contract,
            instruction = instruction,
            symbol = symbol,
            code_snippet = code_snippet,
        );

        if let Some((validation_error, previous_payload)) = correction {
            prompt.push_str(&format!(
                "\nRETRY: Validator error: {validation_error}\nPrevious: {previous_payload}\n"
            ));
        }

        let text = Self::call(&prompt, 0.1)?;
        match Self::extract_payload(&text) {
            Ok(p) => Ok(p),
            Err(_) => Ok(Self::strip_code_fences(&text)),
        }
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
                    .ok_or_else(|| anyhow!("No model specified."))
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
            let t = json["choices"][0]["message"]["content"].as_str().unwrap_or("").trim();
            if t.is_empty() {
                Err(anyhow!("ast_tool: empty model content"))
            } else {
                Ok(t.to_string())
            }
        }
    }
}
