use anyhow::{anyhow, Result};
use serde_json::json;
use std::env;
use std::thread;
use std::time::Duration;

pub struct ApiClient;

struct LlmEndpoint {
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
        let lower = model.to_ascii_lowercase();

        // 1. Google Cloud
        if lower.contains("gemini")
            || (lower.contains("gemma") && !lower.contains("e4b") && !lower.contains("google/"))
        {
            let api_key = env::var("GEMINI_API_KEY")
                .or_else(|_| env::var("GOOGLE_API_KEY"))
                .map_err(|_| {
                    anyhow!("GEMINI_API_KEY or GOOGLE_API_KEY required for Google Cloud models")
                })?;
            return Ok(LlmEndpoint {
                model,
                url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                    .to_string(),
                api_key,
            });
        }

        // 2. NVIDIA NIM Cloud
        if lower.contains("nvidia")
            || lower.contains("nemotron")
            || lower.contains("deepseek")
            || lower.contains("meta/")
            || lower.contains("moonshotai/")
            || lower.starts_with("google/")
        {
            let api_key = env::var("NVIDIA_API_KEY")
                .map_err(|_| anyhow!("NVIDIA_API_KEY required for NVIDIA models"))?;
            return Ok(LlmEndpoint {
                model,
                url: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
                api_key,
            });
        }

        // 3. Local S23 Edge LiteRT-LM
        let base_url = env::var("OPENAI_BASE_URL")
            .or_else(|_| env::var("NEXUS_API_GATEWAY"))
            .unwrap_or_else(|_| "http://192.168.4.223:9379/v1".to_string());

        let clean = base_url.trim_end_matches('/');
        let url = if clean.ends_with("/v1/chat/completions") || clean.ends_with("/chat/completions")
        {
            clean.to_string()
        } else {
            format!("{}/chat/completions", clean)
        };
        let api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| "local-s23-device".to_string());

        Ok(LlmEndpoint { model, url, api_key })
    }

    fn call(prompt: &str, temperature: f32) -> Result<String> {
        let ep = Self::resolve_endpoint()?;
        let body = json!({
            "model": ep.model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": temperature,
            "max_tokens": 512
        });

        let req = ureq::post(&ep.url)
            .timeout(Duration::from_secs(180))
            .set("Content-Type", "application/json")
            .set("Authorization", &format!("Bearer {}", ep.api_key));

        Self::extract_text(req.send_json(body)?)
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
                    let backoff_ms = 1000u64 * (attempt as u64);
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
        let mut in_string = false;
        let mut prev = ' ';
        for c in raw.chars() {
            if c == '"' && prev != '\\' {
                in_string = !in_string;
            }
            if in_string && (c == '\n' || c == '\r') {
                out.push_str("\\n");
            } else {
                out.push(c);
            }
            prev = c;
        }
        out
    }

    fn strip_code_fences(raw: &str) -> String {
        let mut t = raw.trim();
        for lang_tag in
            &["```php", "```python", "```py", "```javascript", "```js", "```json", "```"]
        {
            if let Some(rest) = t.strip_prefix(lang_tag) {
                t = rest;
                break;
            }
        }
        if let Some(body) = t.strip_suffix("```") {
            t = body;
        }
        t.trim().to_string()
    }

    fn extract_payload(text: &str) -> Result<String> {
        if let Some(start) = text.find("<payload>") {
            let actual_start = start + "<payload>".len();
            let end = text[actual_start..]
                .find("</payload>")
                .map(|idx| actual_start + idx)
                .unwrap_or(text.len());
            let inner = &text[actual_start..end];
            let cleaned = Self::strip_code_fences(inner);
            if !cleaned.trim().is_empty() {
                return Ok(cleaned);
            }
        }
        // Fallback if model omitted <payload> tags
        let cleaned = Self::strip_code_fences(text);
        if cleaned.is_empty() {
            Err(anyhow!("Model returned empty output"))
        } else {
            Ok(cleaned)
        }
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
        thread::sleep(Duration::from_millis(300));

        let prompt = format!(
            "Role: Security Triage AI.\n\
             Task: Analyze if this code contains a {finding_type} vulnerability.\n\
             Snippet:\n\
             ```\n{code_snippet}\n```\n\
             Directives:\n\
             - If untrusted input reaches a dangerous sink without sanitization or parameterization: return EXPLOITABLE.\n\
             - If safe, hardcoded, or properly parameterized: return SAFE.\n\
             - If the snippet does not show a dangerous sink being reached at all (e.g. no query execution, no shell/process call, no HTTP request), return UNKNOWN, not EXPLOITABLE.\n\
             - CONSISTENCY REQUIREMENT: \"state\" must agree with \"reason\". If your reason says no sink is visible, or that you cannot confirm exploitability, state MUST be UNKNOWN or SAFE, never EXPLOITABLE. If severity is \"Low\" or \"None\", state MUST NOT be EXPLOITABLE.\n\
             Output ONLY a JSON object inside <payload></payload> tags:\n\
             <payload>{{\"state\": \"EXPLOITABLE\", \"severity\": \"High\", \"reason\": \"Vulnerable sink detected\"}}</payload>"
        );

        let text = Self::call(&prompt, 0.0)?;
        let start = text
            .find("<payload>")
            .map(|i| i + "<payload>".len())
            .unwrap_or_else(|| text.find('{').unwrap_or(0));
        let end = text
            .rfind("</payload>")
            .unwrap_or_else(|| text.rfind('}').map(|i| i + 1).unwrap_or(text.len()))
            .max(start);
        let json_str = Self::strip_code_fences(&text[start..end]);
        let sanitized = Self::sanitize_json(&json_str);

        let parsed: serde_json::Value = serde_json::from_str(&sanitized)
            .unwrap_or_else(|_| serde_json::json!({"state": "UNKNOWN", "severity": "Unknown", "reason": "Triage response failed to parse - model output was not valid JSON."}));

        let state = match parsed["state"].as_str().unwrap_or("").trim().to_uppercase().as_str() {
            "EXPLOITABLE" => "EXPLOITABLE",
            "SAFE" => "SAFE",
            _ => "UNKNOWN",
        };

        Ok((
            state.to_string(),
            parsed["severity"].as_str().unwrap_or("High").to_string(),
            parsed["reason"].as_str().unwrap_or("Tainted dangerous sink").to_string(),
        ))
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
        thread::sleep(Duration::from_millis(300));

        let is_php = lang.eq_ignore_ascii_case("PHP");
        let is_python = lang.eq_ignore_ascii_case("Python");
        let is_js =
            lang.eq_ignore_ascii_case("JavaScript") || lang.eq_ignore_ascii_case("TypeScript");
        let is_rust = lang.eq_ignore_ascii_case("Rust");

        let (rules, exemplar) = if is_php {
            let is_if_sink = (code_snippet.contains("if (")
                || code_snippet.contains("if(!")
                || code_snippet.contains("if (!"))
                && code_snippet.trim_end().ends_with(')');
            let is_secondary_sink = code_snippet.trim().starts_with("$result = mysqli_query")
                || code_snippet.trim().starts_with("$result=mysqli_query");

            let rule = if is_if_sink {
                "MANDATORY: The line being replaced ends with an 'if (...)'. Your replacement MUST end with 'if (!$stmt->execute())'. Do NOT add an opening brace '{' after it."
            } else if is_secondary_sink {
                "MANDATORY: Output ONLY '$result = $stmt->get_result();'"
            } else {
                "MANDATORY: Write out prepared statement ($con->prepare, bind_param, execute, $result = $stmt->get_result())."
            };

            let ex = "[EXAMPLE INPUT]:\n$q = \"SELECT * FROM users WHERE id = '\".$id.\"'\";\nif (!mysqli_query($con, $q))\n[EXAMPLE OUTPUT]:\n<payload>$stmt = $con->prepare(\"SELECT * FROM users WHERE id = ?\");\n$stmt->bind_param(\"s\", $id);\nif (!$stmt->execute())</payload>";
            (rule, ex)
        } else if is_python {
            let rule = "MANDATORY: If fixing SQLi, use parameterized query tuples (e.g. cursor.execute('SELECT * FROM users WHERE id = %s', (val,))). If fixing eval() code execution, replace with safe ast.literal_eval() or safe parsing. If fixing command injection, use subprocess.run(['cmd', arg]) without shell=True.";
            let ex = "[EXAMPLE INPUT]:\ncursor.execute(f\"SELECT * FROM users WHERE name = '{name}'\")\n[EXAMPLE OUTPUT]:\n<payload>cursor.execute(\"SELECT * FROM users WHERE name = %s\", (name,))</payload>";
            (rule, ex)
        } else if is_js {
            let rule = "MANDATORY: If fixing SQLi, use parameterized queries (e.g. db.query('SELECT * FROM users WHERE id = $1', [id])). If fixing command injection, use child_process.execFile('cmd', [args]) instead of exec(string).";
            let ex = "[EXAMPLE INPUT]:\ndb.query(`SELECT * FROM users WHERE id = ${id}`);\n[EXAMPLE OUTPUT]:\n<payload>db.query('SELECT * FROM users WHERE id = $1', [id]);</payload>";
            (rule, ex)
        } else if is_rust {
            let rule = "MANDATORY: For SQL injection, use sqlx::query!() or sqlx::query() with bind() parameters. For command injection, use Command::new(...).args(...), never shell execution such as sh -c with an interpolated command string.";
            let ex = "[EXAMPLE INPUT]:\nlet query = format!(\"SELECT * FROM users WHERE id = {}\", id);\nsqlx::query(&query).execute(&pool).await?;\n[EXAMPLE OUTPUT]:\n<payload>sqlx::query(\"SELECT * FROM users WHERE id = ?\").bind(id).execute(&pool).await?;</payload>";
            (rule, ex)
        } else {
            ("MANDATORY: Replace unsafe dynamic construction with secure parameterized API.", "")
        };

        let prompt = format!(
            "Role: Security Remediation Engine.\n\
             Task: Fix {finding_type} in this {lang} code.\n\n\
             {exemplar}\n\n\
             CRITICAL RULES:\n\
             1. Output ONLY the raw {lang} replacement code inside <payload></payload> tags.\n\
             2. Never use non-existent variables. Only use variables present in the snippet.\n\
             3. Do not output conversational preamble or markdown fences.\n\
             4. {rules}\n\n\
             Snippet to replace:\n\
             ```\n{code_snippet}\n```\n\
             Replacement:"
        );

        let text = Self::call(&prompt, 0.0)?;
        Self::extract_payload(&text)
    }

    pub fn generate_optimization(symbol: &str, code_snippet: &str) -> Result<String> {
        // Constrain the model to crates already visible in the snippet.
        let existing_uses: Vec<&str> = code_snippet
            .lines()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("use ") || t.starts_with("pub use ")
            })
            .collect();

        let allowed_crates_hint = if existing_uses.is_empty() {
            "No external crates are visible in this snippet. Prefer pure `std` only. Do NOT invent crates.".to_string()
        } else {
            format!(
                "Only these imports are already present in the snippet:\n{}\nYou may reuse them. Do NOT introduce any other crate.",
                existing_uses.join("\n")
            )
        };

        let prompt = format!(
            "Role: Conservative Rust optimization engine.\n\
             Task: Produce a replacement BODY for the function `{symbol}`.\n\
             \n\
             HARD CONSTRAINTS (any violation = invalid payload):\n\
             1. Output ONLY the raw Rust statements that belong inside the function body.\n\
             2. Wrap the entire answer in <payload></payload> tags. Nothing else outside the tags.\n\
             3. Do NOT emit a full `fn ... {{ ... }}` declaration — body statements only.\n\
             4. Do NOT introduce any new external crate. No new `use some_crate::...` for crates not already visible.\n\
             5. Prefer pure `std` when the original implementation is std-only.\n\
             6. Keep the same function signature semantics (return type, error style, async-ness).\n\
             7. No comments, no markdown fences, no explanation text.\n\
             8. If you cannot improve the body without new dependencies, return the original body unchanged.\n\
             \n\
             {allowed_crates_hint}\n\
             \n\
             Original function (context only — emit BODY only):\n\
             ```rust\n\
             {code_snippet}\n\
             ```\n\
             \n\
             Replacement body:",
            symbol = symbol,
            allowed_crates_hint = allowed_crates_hint,
            code_snippet = code_snippet,
        );

        let text = Self::with_retry_transient(|| Self::call(&prompt, 0.0), 3)?;
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
        thread::sleep(Duration::from_millis(300));

        let strategy_contract = match strategy {
            "RenameSymbol" => "Payload must be a single valid Rust identifier.",
            "ReplaceFunctionBody" => {
                "Payload must replace the target function body. Use a complete body block for brace-delimited languages; use statements for Python."
            },
            "MakePublic" => "Payload must be empty.",
            _ => "Minimal payload.",
        };

        let mut prompt = format!(
            "Strategy: \"{strategy}\"\nContract: {strategy_contract}\nInstruction: {instruction}\nSymbol: {symbol}\nSnippet:\n```rust\n{code_snippet}\n```\nWrap payload in <payload></payload>.",
            strategy = strategy,
            strategy_contract = strategy_contract,
            instruction = instruction,
            symbol = symbol,
            code_snippet = code_snippet,
        );

        if let Some((validation_error, previous_payload)) = correction {
            prompt
                .push_str(&format!("\nError: {}\nPrev: {}\n", validation_error, previous_payload));
        }

        let text = Self::call(&prompt, 0.0)?;
        let start = text.find("<payload>").map(|i| i + "<payload>".len()).unwrap_or(0);
        let end = text.rfind("</payload>").unwrap_or(text.len()).max(start);
        Ok(Self::strip_code_fences(&text[start..end]))
    }

    fn resolve_model() -> Result<String> {
        let model = env::var("NEXUS_LIGHT_MODEL")
            .or_else(|_| env::var("NEXUS_MODEL"))
            .unwrap_or_else(|_| "gemma-4-e4b-it".to_string());
        Ok(model)
    }

    fn extract_text(response: ureq::Response) -> Result<String> {
        let json: serde_json::Value = response.into_json()?;
        let msg = &json["choices"][0]["message"];

        // Handle OpenAI, Nemotron reasoning models, and standard assistant messages
        if let Some(content) = msg["content"].as_str() {
            if !content.trim().is_empty() {
                return Ok(content.to_string());
            }
        }

        if let Some(rc) = msg["reasoning_content"].as_str() {
            if !rc.trim().is_empty() {
                return Ok(rc.to_string());
            }
        }

        Err(anyhow!("Model returned empty content payload"))
    }
}
