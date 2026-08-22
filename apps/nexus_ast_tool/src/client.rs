use anyhow::{anyhow, Result};
use serde_json::json;
use std::env;
use std::thread;
use std::time::Duration;

pub struct ApiClient;

impl ApiClient {
    pub fn generate_optimization(symbol: &str, code_snippet: &str) -> anyhow::Result<String> {
        let model = Self::resolve_model()?;
        let api_key = env::var("GEMINI_API_KEY").unwrap_or_default();
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );

        let prompt = format!(
            "You are the Nexus Singularity Architect. Optimize this Rust function '{}' for better performance, readability, or idiomatic Rust.\n\nSource:\n```rust\n{}\n```\n\nReturn ONLY the optimized Rust statements for the replacement function body wrapped EXACTLY in <payload> and </payload> tags. Do not include outer braces.",
            symbol, code_snippet
        );

        let body = json!({ "contents": [{ "parts": [{ "text": prompt }] }] });
        let response = ureq::post(&endpoint)
            .set("Content-Type", "application/json")
            .send_json(body)?;

        let json: serde_json::Value = response.into_json()?;
        let text = json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let start = text
            .find("<payload>")
            .ok_or_else(|| anyhow::anyhow!("Missing <payload>"))?
            + 9;
        let end = text
            .rfind("</payload>")
            .ok_or_else(|| anyhow::anyhow!("Missing </payload>"))?;
        Ok(text[start..end]
            .replace("```rust\n", "")
            .replace("```python", "").replace("```php", "").replace("```\n", "").replace("```", "")
            .trim()
            .to_string())
    }
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

    /// Retries a fallible closure with exponential backoff (1s, 2s, 4s).
    fn with_retry<T, F: FnMut() -> Result<T>>(mut f: F, max_attempts: u32) -> Result<T> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match f() {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if attempt >= max_attempts {
                        return Err(e);
                    }
                    let backoff_ms = 1000u64 * 2u64.pow(attempt - 1);
                    eprintln!(
                        "        [🔄 Retry] attempt {} failed: {}. Backing off {}ms...",
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

    pub fn triage_vulnerability_batch(findings_json: &str) -> Result<String> {
        thread::sleep(Duration::from_millis(2000));

        let model = Self::resolve_model()?;
        let model_lower = model.to_lowercase();
        let is_gemini = model_lower.contains("gemini") || model_lower.contains("gemma");

        let api_key = if is_gemini {
            env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY required"))?
        } else {
            env::var("NVIDIA_API_KEY").map_err(|_| anyhow!("NVIDIA_API_KEY required"))?
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
            "You are an elite Security Triage AI. Analyze the following JSON array of code snippets for vulnerabilities.\n\
            For each snippet, determine if it is exploitable based on the provided context.\n\
            Input JSON Array:\n\
            ```json\n\
            {}\n\
            ```\n\
            Return ONLY a JSON array of results wrapped in <payload> tags.\n\
            The array must match the input length and format:\n\
            [{{\"id\": 0, \"is_exploitable\": true, \"severity\": \"High\", \"reason\": \"...\"}}]",
            findings_json
        );

        let body = if is_gemini {
            json!({ "contents": [{ "parts": [{ "text": prompt }] }] })
        } else {
            json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "temperature": 0.1
            })
        };

        let mut req = ureq::post(&endpoint).set("Content-Type", "application/json");
        if !is_gemini {
            req = req.set("Authorization", &format!("Bearer {}", api_key));
        }

        let response = req
            .send_json(body)
            .map_err(|e| anyhow!("HTTP req failed: {e}"))?;
        let text = Self::extract_text(response, is_gemini)?;

        let start = text.find("<payload>").map(|i| i + 9).unwrap_or_else(|| text.find("{").unwrap_or(0));
        let end = text.rfind("</payload>").unwrap_or_else(|| text.rfind("}").map(|i| i + 1).unwrap_or(text.len()));

        Ok(text[start..end]
            .replace("```json\n", "")
            .replace("```python", "").replace("```php", "").replace("```\n", "").replace("```", "")
            .trim()
            .to_string())
    }

    pub fn triage_vulnerability(finding_type: &str, code_snippet: &str) -> Result<(String, String, String)> {
        Self::with_retry(|| Self::triage_vulnerability_attempt(finding_type, code_snippet), 3)
    }

    fn triage_vulnerability_attempt(
        finding_type: &str,
        code_snippet: &str,
    ) -> Result<(String, String, String)> {
        thread::sleep(Duration::from_millis(2000));

        let model = Self::resolve_model()?;
        let model_lower = model.to_lowercase();
        let is_gemini = model_lower.contains("gemini") || model_lower.contains("gemma");

        let api_key = if is_gemini {
            env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY required"))?
        } else {
            env::var("NVIDIA_API_KEY").map_err(|_| anyhow!("NVIDIA_API_KEY required"))?
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
            "You are an elite Security Triage AI.\nAnalyze this code for a '{}' vulnerability.\nSource:\n```\n{}\n```\n\
             CRITICAL DIRECTIVES:\n\
             1. If definitely vulnerable and untrusted data reaches the sink, return \"EXPLOITABLE\".\n\
             2. If definitely mitigated (e.g. prepared statement, integer cast), return \"SAFE\".\n\
             3. If you must make ANY ASSUMPTION about external data validation, session trust, provenance, or sanitization, return \"UNKNOWN\".
\
             4. \"SAFE\" is permitted ONLY when the supplied source proves the sink is mitigated without assumptions.
\
             5. The state MUST be exactly one of: \"EXPLOITABLE\", \"SAFE\", \"UNKNOWN\".
\
             6. Never return \"FALSE POSITIVE\", \"TRUE\", \"FALSE\", or any other state.
\
             Return ONLY a JSON object wrapped in <payload></payload> tags like this:
{{\"state\": \"EXPLOITABLE\", \"severity\": \"High\", \"reason\": \"...\"}}",
            finding_type, code_snippet
        );

        let body = if is_gemini {
            json!({ "contents": [{ "parts": [{ "text": prompt }] }] })
        } else {
            json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "temperature": 0.1
            })
        };

        let mut req = ureq::post(&endpoint).set("Content-Type", "application/json");
        if !is_gemini {
            req = req.set("Authorization", &format!("Bearer {}", api_key));
        }

        let response = req
            .send_json(body)
            .map_err(|e| anyhow!("HTTP req failed: {e}"))?;
        let text = Self::extract_text(response, is_gemini)?;

        let start = text.find("<payload>").map(|i| i + 9).unwrap_or_else(|| text.find("{").unwrap_or(0));
        let end = text.rfind("</payload>").unwrap_or_else(|| text.rfind("}").map(|i| i + 1).unwrap_or(text.len()));
        let json_str = text[start..end]
            .replace("```json\n", "")
            .replace("```python", "").replace("```php", "").replace("```\n", "").replace("```", "")
            .trim()
            .to_string();
        let sanitized = Self::sanitize_json(&json_str);

        let parsed: serde_json::Value =
            serde_json::from_str(&sanitized).map_err(|e| anyhow!("JSON Parse Error: {}", e))?;
        let state = match parsed["state"].as_str().unwrap_or("UNKNOWN") {
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

    pub fn generate_security_patch(
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
            "You are an elite Security Auto-Healer AI.\n\
             Fix this '{}' vulnerability in this {} code:\n\
             ```\n\
             {}\n\
             ```\n\
             CRITICAL: You must use a SEARCH/REPLACE block. Identify the EXACT vulnerable lines to remove (including exact indentation) and provide the secure replacement.\n\
             Output EXACTLY in this format:\n\
             <<<<<<< SEARCH\n\
             [exact code to remove]\n\
             =======\n\
             [secure replacement code]\n\
             >>>>>>> REPLACE\n\
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

        let search_start = text.find("<<<<<<< SEARCH").ok_or_else(|| anyhow::anyhow!("Missing <<<<<<< SEARCH in payload"))?;
        let delim = text.find("=======").ok_or_else(|| anyhow::anyhow!("Missing ======= in payload"))?;
        let replace_end = text.find(">>>>>>> REPLACE").ok_or_else(|| anyhow::anyhow!("Missing >>>>>>> REPLACE in payload"))?;

        let search_block = text[search_start + 14..delim].trim_matches('\n').to_string();
        let replace_block = text[delim + 7..replace_end].trim_matches('\n').to_string();

        Ok((search_block, replace_block))
    }

    fn resolve_model() -> Result<String> {
        env::var("NEXUS_LIGHT_MODEL")
            .or_else(|_| env::var("NEXUS_MODEL"))
            .or_else(|_| {
                env::var("NEXUS_LIGHT_CASCADE")
                    .ok()
                    .and_then(|cascade| {
                        cascade
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .find(|s| !s.is_empty())
                    })
                    .ok_or_else(|| {
                        anyhow!(
                            "No model specified. Set NEXUS_LIGHT_MODEL, NEXUS_MODEL, or NEXUS_LIGHT_CASCADE"
                        )
                    })
            })
    }

    fn query_model_internal(
        instruction: &str,
        symbol: &str,
        code_snippet: &str,
        strategy: &str,
        correction: Option<(&str, &str)>,
    ) -> Result<String> {
        Self::with_retry(
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

        let model = Self::resolve_model()?;
        let model_lower = model.to_lowercase();
        let is_gemini = model_lower.contains("gemini") || model_lower.contains("gemma");

        let api_key = if is_gemini {
            env::var("GEMINI_API_KEY").map_err(|_| anyhow!("GEMINI_API_KEY required for Gemini models"))?
        } else {
            env::var("NVIDIA_API_KEY").map_err(|_| anyhow!("NVIDIA_API_KEY required for non-Gemini models"))?
        };

        let endpoint = if is_gemini {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model, api_key
            )
        } else {
            "https://integrate.api.nvidia.com/v1/chat/completions".to_string()
        };

        let strategy_contract = match strategy {
            "RenameSymbol" => {
                "Payload must be a single valid Rust identifier (the new name). \
                 No quotes, no keywords, no punctuation."
            }
            "ReplaceFunctionBody" => {
                "Payload must be the exact statements that will replace the function body. \
                 Do NOT include the outer braces, the fn signature, or any surrounding code."
            }
            "MakePublic" => {
                "Payload must be empty (the strategy ignores it). Emit nothing between the tags."
            }
            "InsertEarlyReturn" => {
                "Payload must be a valid Rust expression that will be returned \
                 (e.g. `()` or `None` or `Err(...)`). \
                 Do not include the `return` keyword or the trailing semicolon."
            }
            "InsertComment" => {
                "Payload must be the comment text only (no // or /* */ markers)."
            }
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
            Source context (for reference only – do not emit the whole function unless the strategy requires body statements):\n\
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
                Produce a corrected payload that satisfies the strategy contract above. Still wrap it ONLY in <payload></payload> tags.\n"
            ));
        }

        let body = if is_gemini {
            json!({ "contents": [{ "parts": [{ "text": prompt }] }] })
        } else {
            json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "temperature": 0.1
            })
        };

        let mut req = ureq::post(&endpoint).set("Content-Type", "application/json");
        if !is_gemini {
            req = req.set("Authorization", &format!("Bearer {}", api_key));
        }

        let response = req
            .send_json(body)
            .map_err(|e| anyhow!("HTTP request failed: {e}"))?;
        let text = Self::extract_text(response, is_gemini)?;

        let start = text.find("<payload>").map(|i| i + 9).unwrap_or(0);
        let end = text.rfind("</payload>").unwrap_or(text.len());

        let payload = text[start..end]
            .replace("```rust\n", "")
            .replace("```rust", "")
            .replace("```\n", "")
            .replace("```python", "").replace("```php", "").replace("```\n", "").replace("```", "")
            .trim()
            .to_string();

        Ok(payload)
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
