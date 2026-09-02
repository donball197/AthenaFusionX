// Auto-generated AST Auto-Heal Sanitizer
pub fn sanitize_model_patch(raw_response: &str, is_php: bool) -> String {
    let trimmed = raw_response.trim();

    // 1. Extract innermost markdown code fence if present
    let code_block = if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        let content_start = after_fence.find('\n').map(|idx| idx + 1).unwrap_or(0);
        let inner = &after_fence[content_start..];
        if let Some(end) = inner.rfind("```") {
            inner[..end].trim()
        } else {
            inner.trim()
        }
    } else {
        trimmed
    };

    // 2. Strip natural language lines that precede actual code
    let mut cleaned_lines = Vec::new();
    let mut code_started = false;

    for line in code_block.lines() {
        let l_trim = line.trim();

        if !code_started
            && (l_trim.starts_with('$')
                || l_trim.starts_with("<?php")
                || l_trim.starts_with("if")
                || l_trim.starts_with("while")
                || l_trim.starts_with("for")
                || l_trim.starts_with("try")
                || l_trim.starts_with("return")
                || l_trim.starts_with("function")
                || l_trim.starts_with("class")
                || l_trim.starts_with("const")
                || l_trim.starts_with("let")
                || l_trim.starts_with("var")
                || l_trim.starts_with("use")
                || l_trim.starts_with("//")
                || l_trim.starts_with("/*")
                || l_trim.ends_with(';')
                || l_trim.ends_with('{'))
        {
            code_started = true;
        }

        if code_started {
            cleaned_lines.push(line);
        }
    }

    let mut result = if cleaned_lines.is_empty() {
        code_block.to_string()
    } else {
        cleaned_lines.join("\n")
    };

    // 3. Structural Control-Flow Normalization:
    // If the patch ends with an `if (...) {` header, trim trailing `{` so it splices cleanly before an existing `{`
    let trimmed_end = result.trim_end();
    if (trimmed_end.contains("if (") || trimmed_end.contains("if(!") || trimmed_end.contains("if (!"))
        && trimmed_end.ends_with('{')
    {
        result = trimmed_end.trim_end_matches('{').trim_end().to_string();
    }

    // 4. For PHP snippets embedded in HTML templates:
    if is_php
        && result.starts_with("<?php")
        && !raw_response.contains("<!DOCTYPE")
        && !raw_response.contains("<html")
    {
        let lines: Vec<&str> = result.lines().collect();
        if lines.len() > 1 && lines[0].trim() == "<?php" {
            let mut stripped = lines[1..].join("\n");
            if stripped.trim_end().ends_with("?>") {
                if let Some(pos) = stripped.rfind("?>") {
                    stripped = stripped[..pos].trim_end().to_string();
                }
            }
            return stripped;
        }
    }

    result
}
