import sys

path = "apps/nexus_ast_tool/src/polyglot.rs"

with open(path, "r") as f:
    content = f.read()

# --- 1. Add patch_snippet field to SecurityFinding ---
old1 = '''#[derive(Debug, Clone)]
pub struct SecurityFinding {
    pub language: SupportedLanguage,
    pub pattern_name: String,
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub source_snippet: String,
}'''
new1 = '''#[derive(Debug, Clone)]
pub struct SecurityFinding {
    pub language: SupportedLanguage,
    pub pattern_name: String,
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub source_snippet: String,
    /// Exact text of start_line..=end_line only, no surrounding context.
    /// Must be used as the auto-heal LLM's replacement target instead of
    /// source_snippet, because PolyglotSplicer::splice only ever cuts out
    /// start_line..end_line -- if the model is shown a wider snippet than
    /// that, it writes a wider replacement than splice can correctly insert.
    pub patch_snippet: String,
}'''

count1 = content.count(old1)
if count1 != 1:
    print(f"ANCHOR MISMATCH (struct): found {count1}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old1, new1, 1)

# --- 2. Compute patch_snippet in parse_security and populate the new field ---
old2 = '''                let context_start = start_line.saturating_sub(20);
                let context_end = (end_line + 3).min(source_lines.len().saturating_sub(1));
                let snippet = if start_line < source_lines.len() {
                    source_lines[context_start..=context_end].join("\\n")
                } else { String::new() };

                results.push(SecurityFinding {
                    language: lang,
                    pattern_name: rule_id.to_string(), 
                    path: path.to_path_buf(),
                    start_line: start_line + 1,
                    end_line: end_line + 1,
                    source_snippet: snippet,
                });'''
new2 = '''                let context_start = start_line.saturating_sub(20);
                let context_end = (end_line + 3).min(source_lines.len().saturating_sub(1));
                let snippet = if start_line < source_lines.len() {
                    source_lines[context_start..=context_end].join("\\n")
                } else { String::new() };

                // Narrow, exact-line-range snippet used ONLY as the auto-heal
                // replacement target. Must match PolyglotSplicer::splice's
                // start_line..end_line range exactly, or the LLM writes a patch
                // sized to the wide `snippet` above while splice only cuts out
                // this narrow range -- guaranteeing duplicated context and a
                // syntax-check failure on every auto-heal attempt.
                let patch_target = if end_line < source_lines.len() {
                    source_lines[start_line..=end_line].join("\\n")
                } else { String::new() };

                results.push(SecurityFinding {
                    language: lang,
                    pattern_name: rule_id.to_string(), 
                    path: path.to_path_buf(),
                    start_line: start_line + 1,
                    end_line: end_line + 1,
                    source_snippet: snippet,
                    patch_snippet: patch_target,
                });'''

count2 = content.count(old2)
if count2 != 1:
    print(f"ANCHOR MISMATCH (parse_security push): found {count2}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old2, new2, 1)

# --- 3. Cross-rule dedup, re-applied (confirmed absent on 3 separate checks now) ---
old3 = '''    pub fn scan_security_vulnerabilities<P: AsRef<Path>>(root: P, rules: &[SecurityRule]) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(file_lang) = SupportedLanguage::from_path(path) {
                if let Ok(content) = fs::read_to_string(path) {
                    for rule in rules {
                        if SupportedLanguage::from_string(&rule.language) == Some(file_lang) {
                            if let Ok(mut discovered) = Self::parse_security(path, &content, file_lang, &rule.tree_sitter_query, &rule.id) {
                                findings.append(&mut discovered);
                            }
                        }
                    }
                }
            }
        }
        findings
    }'''
new3 = '''    pub fn scan_security_vulnerabilities<P: AsRef<Path>>(root: P, rules: &[SecurityRule]) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(file_lang) = SupportedLanguage::from_path(path) {
                if let Ok(content) = fs::read_to_string(path) {
                    for rule in rules {
                        if SupportedLanguage::from_string(&rule.language) == Some(file_lang) {
                            if let Ok(mut discovered) = Self::parse_security(path, &content, file_lang, &rule.tree_sitter_query, &rule.id) {
                                findings.append(&mut discovered);
                            }
                        }
                    }
                }
            }
        }

        // Two rule files can declare the same rule id, which silently doubles
        // every finding and every AI-triage call. Collapse exact
        // (path, line, rule id) duplicates as a safety net.
        findings.sort_by(|a, b| {
            (a.path.as_path(), a.start_line, a.pattern_name.as_str())
                .cmp(&(b.path.as_path(), b.start_line, b.pattern_name.as_str()))
        });
        findings.dedup_by(|a, b| {
            a.path == b.path && a.start_line == b.start_line && a.pattern_name == b.pattern_name
        });

        findings
    }'''

count3 = content.count(old3)
if count3 != 1:
    print(f"ANCHOR MISMATCH (scan_security_vulnerabilities): found {count3}", file=sys.stderr)
    sys.exit(1)
content = content.replace(old3, new3, 1)

with open(path, "w") as f:
    f.write(content)

print("OK: patch_snippet field added + populated, cross-rule dedup re-applied")
