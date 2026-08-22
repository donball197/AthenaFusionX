import sys

path = "apps/nexus_ast_tool/src/client.rs"

with open(path, "r") as f:
    content = f.read()

# --- 1. Add with_retry + sanitize_json helpers right after `impl ApiClient {` (new block) ---
anchor_impl = "impl ApiClient {\n    pub fn new() -> Self {\n        Self\n    }"
if content.count(anchor_impl) != 1:
    print(f"ANCHOR MISMATCH (impl block): found {content.count(anchor_impl)}", file=sys.stderr)
    sys.exit(1)

helpers = anchor_impl + '''

    /// Retries a fallible closure with exponential backoff (1s, 2s, 4s).
    /// Without this, a single transient 429 or a malformed-JSON response from
    /// the model permanently drops that finding from the report.
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
                        "        [\\u{1F501} Retry] attempt {} failed: {}. Backing off {}ms...",
                        attempt, e, backoff_ms
                    );
                    thread::sleep(Duration::from_millis(backoff_ms));
                }
            }
        }
    }

    /// The model's JSON reply can contain raw backslashes echoed from the source
    /// snippet (e.g. a Windows path or regex in the vulnerable code), which are
    /// not valid JSON escapes and break serde_json::from_str. Escape any
    /// backslash that isn't already part of a valid escape sequence.
    fn sanitize_json(raw: &str) -> String {
        let mut out = String::with_capacity(raw.len());
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\\\' {
                match chars.peek() {
                    Some('"') | Some('\\\\') | Some('/') | Some('n') | Some('t')
                    | Some('r') | Some('b') | Some('f') | Some('u') => out.push(c),
                    _ => {
                        out.push('\\\\');
                        out.push('\\\\');
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }'''

content = content.replace(anchor_impl, helpers, 1)

# --- 2. Wrap triage_vulnerability body in with_retry + sanitize before parse ---
old_triage_sig = "    pub fn triage_vulnerability(finding_type: &str, code_snippet: &str) -> Result<(bool, String, String)> {\n        // Anti-429 Throttle\n        thread::sleep(Duration::from_millis(2000));\n        \n        let model ="
new_triage_sig = "    pub fn triage_vulnerability(finding_type: &str, code_snippet: &str) -> Result<(bool, String, String)> {\n        Self::with_retry(|| Self::triage_vulnerability_attempt(finding_type, code_snippet), 3)\n    }\n\n    fn triage_vulnerability_attempt(finding_type: &str, code_snippet: &str) -> Result<(bool, String, String)> {\n        // Anti-429 Throttle\n        thread::sleep(Duration::from_millis(2000));\n        \n        let model ="

if content.count(old_triage_sig) != 1:
    print(f"ANCHOR MISMATCH (triage_vulnerability sig): found {content.count(old_triage_sig)}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old_triage_sig, new_triage_sig, 1)

old_parse = '''        let json_str = text[start..end].replace("```json\\n", "").replace("```", "").trim().to_string();

        let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| anyhow!("JSON Parse Error: {}", e))?;'''

new_parse = '''        let json_str = text[start..end].replace("```json\\n", "").replace("```", "").trim().to_string();
        let sanitized = Self::sanitize_json(&json_str);

        let parsed: serde_json::Value = serde_json::from_str(&sanitized).map_err(|e| anyhow!("JSON Parse Error: {}", e))?;'''

if content.count(old_parse) != 1:
    print(f"ANCHOR MISMATCH (json parse): found {content.count(old_parse)}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old_parse, new_parse, 1)

# --- 3. Wrap generate_security_patch the same way ---
old_patch_sig = "    pub fn generate_security_patch(finding_type: &str, code_snippet: &str, lang: &str) -> Result<String> {\n        // Anti-429 Throttle\n        thread::sleep(Duration::from_millis(2000));\n\n        let model ="
new_patch_sig = "    pub fn generate_security_patch(finding_type: &str, code_snippet: &str, lang: &str) -> Result<String> {\n        Self::with_retry(|| Self::generate_security_patch_attempt(finding_type, code_snippet, lang), 3)\n    }\n\n    fn generate_security_patch_attempt(finding_type: &str, code_snippet: &str, lang: &str) -> Result<String> {\n        // Anti-429 Throttle\n        thread::sleep(Duration::from_millis(2000));\n\n        let model ="

if content.count(old_patch_sig) != 1:
    print(f"ANCHOR MISMATCH (generate_security_patch sig): found {content.count(old_patch_sig)}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old_patch_sig, new_patch_sig, 1)

with open(path, "w") as f:
    f.write(content)

print("OK: retry/backoff + JSON sanitize patched into client.rs")
