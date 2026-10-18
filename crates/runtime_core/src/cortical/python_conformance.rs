// Extracted from cortical_loop.rs; function and test bodies preserved.

/// Deterministic conformance checks for a written Python file. Requirements are
/// derived from the objective text:
///   * `name()`                     -> file must define `def name(`
///   * snake_case identifiers       -> must appear in the file
///   * `import ONLY a, b and c`     -> only those top-level modules may be imported
///   * `under|at most N lines`      -> line cap (all lines counted)
///
/// Returns human-readable defects; empty means conformant.
pub(crate) fn python_conformance_defects(objective: &str, path: &str, content: &str) -> Vec<String> {
    let mut defects: Vec<String> = Vec::new();
    let obj_lower = objective.to_ascii_lowercase();

    // 1. functions named as `name()`
    let mut required_fns: Vec<String> = Vec::new();
    let mut from = 0usize;
    while let Some(off) = objective[from..].find("()") {
        let end = from + off;
        let ident_rev: String = objective[..end]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let name: String = ident_rev.chars().rev().collect();
        let starts_ok = name.chars().next().map(|c| !c.is_ascii_digit()).unwrap_or(false);
        let start = end - name.len();
        let before = &objective[..start];
        let is_attr_call = before.ends_with('.');
        let defining = before.split_whitespace().rev().take(3).any(|w| {
            let w = w
                .trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_ascii_lowercase();
            w == "def" || w == "function" || w == "method" || w.starts_with("defin")
        });
        if starts_ok && !is_attr_call && defining && !required_fns.contains(&name) {
            required_fns.push(name);
        }
        from = end + 2;
    }
    for name in &required_fns {
        let def_prefix = format!("def {}(", name);
        let async_prefix = format!("async def {}(", name);
        let defined = content.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with(&def_prefix) || t.starts_with(&async_prefix)
        });
        if !defined {
            defects.push(format!("missing `def {}(...)` required by the objective", name));
        }
    }

    // 2. snake_case identifiers named in the objective must appear in the file
    let path_stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let mut idents: Vec<String> = Vec::new();
    for tok in objective.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        let tok = tok.trim_matches('_');
        if tok.len() < 3 || !tok.contains('_') {
            continue;
        }
        // marker tokens such as WRITE_OK
        if tok.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            continue;
        }
        if tok == path_stem || required_fns.iter().any(|f| f == tok) {
            continue;
        }
        if !idents.iter().any(|i| i == tok) {
            idents.push(tok.to_string());
        }
    }
    for tok in &idents {
        if !content.contains(tok.as_str()) {
            defects.push(format!("missing identifier `{}` named in the objective", tok));
        }
    }

    // 3. `import ONLY a, b and c` allowlist
    if let Some(pos) = obj_lower.find("import only") {
        let after = &objective[pos + "import only".len()..];
        let sentence = after.split(". ").next().unwrap_or(after).trim().trim_end_matches('.');
        let mut allowed: Vec<String> = Vec::new();
        'items: for item in sentence.split(',') {
            let words: Vec<&str> = item.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            let mut i = if words[0].eq_ignore_ascii_case("and") { 1 } else { 0 };
            let mut batch: Vec<String> = Vec::new();
            let mut conforming = true;
            while i < words.len() {
                let w = words[i];
                if !w.is_empty()
                    && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                {
                    batch.push(w.split('.').next().unwrap_or(w).to_string());
                    i += 1;
                    if i < words.len() {
                        if words[i].eq_ignore_ascii_case("and") {
                            i += 1;
                        } else {
                            conforming = false;
                            break;
                        }
                    }
                } else {
                    conforming = false;
                    break;
                }
            }
            if !conforming || batch.is_empty() {
                break 'items;
            }
            allowed.extend(batch);
        }
        if !allowed.is_empty() {
            let mut bad: Vec<String> = Vec::new();
            for line in content.lines() {
                let t = line.trim_start();
                let mut modules: Vec<String> = Vec::new();
                if let Some(rest) = t.strip_prefix("import ") {
                    for part in rest.split(';').next().unwrap_or("").split(',') {
                        let m = part.split_whitespace().next().unwrap_or("");
                        modules.push(m.split('.').next().unwrap_or("").to_string());
                    }
                } else if let Some(rest) = t.strip_prefix("from ") {
                    let m = rest.split_whitespace().next().unwrap_or("");
                    if !m.starts_with('.') {
                        modules.push(m.split('.').next().unwrap_or("").to_string());
                    }
                }
                for m in modules {
                    if !m.is_empty()
                        && m != "__future__"
                        && !allowed.contains(&m)
                        && !bad.contains(&m)
                    {
                        bad.push(m);
                    }
                }
            }
            if !bad.is_empty() {
                defects.push(format!("imports not allowed by the objective: {}", bad.join(", ")));
            }
        }
    }

    // 4. line cap
    let words: Vec<String> = objective
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_ascii_lowercase())
        .collect();
    let mut max_lines: Option<usize> = None;
    for (idx, w) in words.iter().enumerate() {
        if w != "lines" || idx == 0 {
            continue;
        }
        let Ok(n) = words[idx - 1].parse::<usize>() else { continue };
        let prev = if idx >= 2 { words[idx - 2].as_str() } else { "" };
        let prev2 = if idx >= 3 { words[idx - 3].as_str() } else { "" };
        let limit = if matches!(
            (prev2, prev),
            (_, "under") | (_, "below") | ("fewer", "than") | ("less", "than")
        ) {
            Some(n.saturating_sub(1))
        } else if matches!((prev2, prev), ("at", "most") | ("up", "to"))
            || (prev2 == "more" && prev == "than" && idx >= 4 && words[idx - 4] == "no")
        {
            Some(n)
        } else {
            None
        };
        if let Some(l) = limit {
            max_lines = Some(max_lines.map_or(l, |m| m.min(l)));
        }
    }
    if let Some(limit) = max_lines {
        let n_lines = content.lines().count();
        if n_lines > limit {
            defects.push(format!("{} lines exceeds the objective's limit of {}", n_lines, limit));
        }
    }

    defects
}

/// `ast.parse` the file with python3 (no bytecode side effects). Returns None when the file
/// parses, or when python3 is unavailable (the check is skipped, never blocking).
pub(crate) async fn python_syntax_error(path: &str) -> Option<String> {
    let out = tokio::process::Command::new("python3")
        .args([
            "-c",
            "import ast,sys; ast.parse(open(sys.argv[1], encoding='utf-8').read(), sys.argv[1])",
            path,
        ])
        .output()
        .await
        .ok()?;
    if out.status.success() {
        None
    } else {
        Some(
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("syntax error")
                .trim()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod conformance_tests {
    use super::python_conformance_defects;

    const OBJ: &str = "Write a Python script exactly named 'sys_monitor.py' in the current directory. The script must import ONLY json, sys, and psutil, define a function get_metrics() returning a dict with cpu_percent and memory_percent, print it as JSON, and remain under 20 lines.";

    #[test]
    fn rejects_import_only_stub() {
        let d = python_conformance_defects(OBJ, "sys_monitor.py", "import json, sys, psutil");
        assert!(d.iter().any(|x| x.contains("get_metrics")), "{d:?}");
        assert!(d.iter().any(|x| x.contains("cpu_percent")), "{d:?}");
        assert!(d.iter().any(|x| x.contains("memory_percent")), "{d:?}");
    }

    #[test]
    fn accepts_conforming_script() {
        let src = "import json\nimport sys\nimport psutil\n\ndef get_metrics():\n    return {\"cpu_percent\": psutil.cpu_percent(), \"memory_percent\": psutil.virtual_memory().percent}\n\nif __name__ == \"__main__\":\n    json.dump(get_metrics(), sys.stdout)\n";
        let d = python_conformance_defects(OBJ, "sys_monitor.py", src);
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn rejects_forbidden_import_and_long_file() {
        let src = format!("import os\n{}", "x = 1\n".repeat(24));
        let d = python_conformance_defects(OBJ, "sys_monitor.py", &src);
        assert!(d.iter().any(|x| x.contains("imports not allowed")), "{d:?}");
        assert!(d.iter().any(|x| x.contains("exceeds")), "{d:?}");
    }
}

#[cfg(test)]
mod conformance_regression_tests {
    use super::python_conformance_defects;

    #[test]
    fn attribute_calls_in_objective_are_not_required_definitions() {
        let obj = "Write a file named \"sys_monitor_v2.py\" containing EXACTLY this code:\nimport json\nimport os\ndef get_metrics():\n    return {\"load_avg\": os.getloadavg()}\nprint(json.dumps(get_metrics()))";
        let src = "import json\nimport os\ndef get_metrics():\n    return {\"load_avg\": os.getloadavg()}\nprint(json.dumps(get_metrics()))\n";
        let d = python_conformance_defects(obj, "sys_monitor_v2.py", src);
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn stub_is_still_rejected_for_the_exact_code_objective() {
        let obj = "Write a file named \"sys_monitor_v2.py\" containing EXACTLY this code:\nimport json\nimport os\ndef get_metrics():\n    return {\"load_avg\": os.getloadavg()}\nprint(json.dumps(get_metrics()))";
        let d = python_conformance_defects(obj, "sys_monitor_v2.py", "import json import os def get_metrics(): return {");
        assert!(d.iter().any(|x| x.contains("get_metrics")), "{d:?}");
    }
}
