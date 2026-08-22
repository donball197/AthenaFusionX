#!/usr/bin/env bash
# fix-client-prompt.sh  –  restores client.rs and installs the correct heal prompt

set -euo pipefail

CLIENT="apps/nexus_ast_tool/src/client.rs"

# ------------------------------------------------------------------
# 1. Restore a known-good generate_security_patch_attempt function
# ------------------------------------------------------------------
python3 - <<'PY'
from pathlib import Path
import re

path = Path("apps/nexus_ast_tool/src/client.rs")
text = path.read_text()

# Locate the whole function (from "fn generate_security_patch_attempt" up to the next "fn " or end of impl)
pattern = re.compile(
    r'(    pub fn generate_security_patch\([\s\S]*?\) -> Result<String> \{[\s\S]*?\n    \}\n\n)'
    r'(    fn generate_security_patch_attempt\([\s\S]*?\n    \})',
    re.MULTILINE
)

new_func = r'''    fn generate_security_patch_attempt(
        finding_type: &str,
        code_snippet: &str,
        lang: &str,
    ) -> Result<String> {
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

        // Strengthened prompt – exact drop-in replacement for the narrow patch_snippet
        let prompt = format!(
            "You are an elite Security Auto-Healer AI specialised in SQL injection.\n\
             Language: {lang}\n\
             Vulnerability type: {finding_type}\n\n\
             The following lines are the EXACT text that will be cut out of the source file and replaced by your answer:\n\
             ```\n{code_snippet}\n```\n\n\
             HARD CONTRACT (any violation causes automatic rejection):\n\
             1. Output ONLY the replacement lines for the snippet above. Nothing else.\n\
             2. Convert the vulnerable query into a prepared statement / parameterised query.\n\
             3. Keep every variable name that already appears in the snippet. Do NOT invent new identifiers ($status, $id, etc.).\n\
             4. Do NOT emit surrounding context, markdown fences, explanations, or closing braces that were not opened in the snippet.\n\
             5. The replacement must be a pure textual drop-in for the lines shown.\n\n\
             Wrap the code EXACTLY between <payload> and </payload> tags.",
            finding_type = finding_type,
            lang = lang,
            code_snippet = code_snippet
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

        let start = text
            .find("<payload>")
            .ok_or_else(|| anyhow!("Missing <payload>"))?
            + 9;
        let end = text
            .rfind("</payload>")
            .ok_or_else(|| anyhow!("Missing </payload>"))?;
        Ok(text[start..end]
            .replace("```python\n", "")
            .replace("```php\n", "")
            .replace("```", "")
            .trim_end()
            .to_string())
    }'''

m = pattern.search(text)
if not m:
    print("ERROR: could not locate generate_security_patch / generate_security_patch_attempt")
    raise SystemExit(1)

# Keep the public wrapper, replace only the attempt function
new_text = text[:m.start(2)] + new_func + text[m.end(2):]
path.write_text(new_text)
print("client.rs: generate_security_patch_attempt replaced cleanly")
PY

# ------------------------------------------------------------------
# 2. Make sure main.rs still uses the narrow snippet
# ------------------------------------------------------------------
MAIN="apps/nexus_ast_tool/src/main.rs"
sed -i 's/finding\.patch_start_line/finding.start_line/g' "$MAIN"
sed -i 's/&finding\.source_snippet/\&finding.patch_snippet/g' "$MAIN"
sed -i 's/finding\.source_snippet/finding.patch_snippet/g' "$MAIN"

# ------------------------------------------------------------------
# 3. Clear stale cache
# ------------------------------------------------------------------
rm -f nexus_cache.json
echo "cache cleared"

# ------------------------------------------------------------------
# 4. Compile check
# ------------------------------------------------------------------
echo
echo "==> Building..."
cargo build --bin nexus_ast_tool 2>&1 | tail -30

echo
echo "If the build succeeded, re-run:"
echo "  cargo run --bin nexus_ast_tool -- audit \\"
echo "    --repo https://github.com/appsecco/sqlinjection-training-app.git \\"
echo "    --ai-triage --auto-heal --generate-report"
