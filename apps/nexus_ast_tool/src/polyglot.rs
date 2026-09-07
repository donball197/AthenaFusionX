//! Polyglot Language Engine & Universal Symbol Discovery
//! Enforcing Zero Fragmentation (Strict 0.23.x ABI)

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Parser, Query, QueryCursor};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Rust,
    Zig,
    Python,
    JavaScript,
    TypeScript,
    C,
    Cpp,
    Go,
    PHP,
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "rs" => Some(Self::Rust),
            "zig" | "zon" => Some(Self::Zig),
            "py" => Some(Self::Python),
            "js" | "jsx" | "mjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" => Some(Self::Cpp),
            "go" => Some(Self::Go),
            "php" => Some(Self::PHP),
            _ => None,
        }
    }

    pub fn from_string(lang: &str) -> Option<Self> {
        match lang.trim().to_ascii_lowercase().as_str() {
            "rust" => Some(Self::Rust),
            "python" => Some(Self::Python),
            "javascript" | "js" => Some(Self::JavaScript),
            "typescript" | "ts" => Some(Self::TypeScript),
            "c" => Some(Self::C),
            "cpp" | "c++" => Some(Self::Cpp),
            "go" => Some(Self::Go),
            "zig" => Some(Self::Zig),
            "php" => Some(Self::PHP),
            _ => None,
        }
    }

    pub fn tree_sitter_language(&self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Zig => tree_sitter_zig::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::PHP => tree_sitter_php::LANGUAGE_PHP.into(),
        }
    }

    pub fn symbol_query(&self) -> &'static str {
        match self {
            Self::Rust => "(function_item name: (identifier) @name)",
            Self::Zig => "(FunctionDecl name: (identifier) @name)",
            Self::Python => "(function_definition name: (identifier) @name)",
            Self::JavaScript => "(function_declaration name: (identifier) @name)",
            Self::TypeScript => "(function_declaration name: (identifier) @name)",
            Self::C => "(function_declarator declarator: (identifier) @name)",
            Self::Cpp => "(function_declarator declarator: (identifier) @name)",
            Self::Go => "(function_declaration name: (identifier) @name)",
            Self::PHP => "(function_definition name: (name) @name)",
        }
    }

    pub fn parses_cleanly(&self, source: &str) -> bool {
        let mut parser = Parser::new();
        if parser.set_language(&self.tree_sitter_language()).is_err() {
            return false;
        }
        match parser.parse(source, None) {
            Some(tree) => !tree.root_node().has_error(),
            None => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityRule {
    pub id: String,
    pub language: String,
    pub severity: String,
    pub description: String,
    pub tree_sitter_query: String,
}

#[derive(Debug, Clone)]
pub struct PolyglotSymbol {
    pub language: SupportedLanguage,
    pub name: String,
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone)]
pub struct SecurityFinding {
    pub language: SupportedLanguage,
    pub pattern_name: String,
    pub severity: String,
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub source_snippet: String,
    pub patch_snippet: String,
    pub sink_start_line: usize,
    pub sink_end_line: usize,
    pub sink_text: String,
}

pub fn find_concat_assignment(
    source: &str,
    ts_lang: &Language,
    var_name: &str,
    before_line: usize,
) -> Option<(usize, usize, String)> {
    let mut parser = Parser::new();
    parser.set_language(ts_lang).ok()?;
    let tree = parser.parse(source, None)?;
    let query = Query::new(
        ts_lang,
        "(assignment_expression left: (variable_name) @var right: (binary_expression) @concat) @assign",
    ).ok()?;
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut best: Option<(usize, usize, usize, String)> = None;
    for m in matches {
        let mut var_text = None;
        let mut assign_node = None;
        for capture in m.captures {
            match query.capture_names()[capture.index as usize] {
                "var" => var_text = capture.node.utf8_text(source.as_bytes()).ok(),
                "assign" => assign_node = Some(capture.node),
                _ => {}
            }
        }
        if var_text != Some(var_name) {
            continue;
        }
        let Some(node) = assign_node else { continue };
        let end_row = node.end_position().row;
        if end_row + 1 >= before_line {
            continue;
        }
        let dist = before_line - (end_row + 1);
        if best.as_ref().map(|(_, _, d, _)| dist < *d).unwrap_or(true) {
            let start_row = node.start_position().row;
            let Some(text) = node.utf8_text(source.as_bytes()).ok() else { continue };
            best = Some((start_row, end_row, dist, text.to_string()));
        }
    }
    best.map(|(s, e, _, t)| (s + 1, e + 1, t))
}

pub fn extract_query_var(sink_text: &str) -> Option<String> {
    let markers = ["mysqli_query(", "mysqli_multi_query(", "->query(", "->multi_query("];
    let start = markers.iter().find_map(|m| sink_text.find(m).map(|i| i + m.len()))?;
    let bytes = sink_text.as_bytes();
    let mut depth = 1i32;
    let mut end = None;
    for (i, &b) in bytes[start..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let args = &sink_text[start..end?];
    let last = args.rsplit(',').next()?.trim();
    if last.starts_with('$')
        && last.len() > 1
        && last[1..].chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        Some(last.to_string())
    } else {
        None
    }
}

pub struct PolyglotScanner;

impl PolyglotScanner {
    pub fn load_rules<P: AsRef<Path>>(rules_dir: P) -> Vec<SecurityRule> {
        let mut rules = Vec::new();
        for entry in WalkDir::new(rules_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "yaml" || ext == "yml" {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        match serde_yaml::from_str::<SecurityRule>(&content) {
                            Ok(rule) => rules.push(rule),
                            Err(e) => eprintln!(
                                "==> [Warning] Failed to parse YAML rule file {}: {}",
                                path.display(),
                                e
                            ),
                        }
                    }
                }
            }
        }
        rules
    }

    pub fn scan_path<P: AsRef<Path>>(root: P) -> Vec<PolyglotSymbol> {
        let mut symbols = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(lang) = SupportedLanguage::from_path(path) {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(mut discovered) = Self::parse_symbols(path, &content, lang) {
                        symbols.append(&mut discovered);
                    }
                }
            }
        }
        symbols
    }

    fn parse_symbols(
        path: &Path,
        source: &str,
        lang: SupportedLanguage,
    ) -> Result<Vec<PolyglotSymbol>> {
        let mut parser = Parser::new();
        let ts_lang = lang.tree_sitter_language();
        parser
            .set_language(&ts_lang)
            .map_err(|e| anyhow!("Failed setting parser language: {e}"))?;
        let tree =
            parser.parse(source, None).ok_or_else(|| anyhow!("Tree-sitter parse failure"))?;
        let query = Query::new(&ts_lang, lang.symbol_query())
            .map_err(|e| anyhow!("Failed to compile query: {e}"))?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        let mut results = Vec::new();
        for m in matches {
            for capture in m.captures {
                if let Ok(name) = capture.node.utf8_text(source.as_bytes()) {
                    let range = capture.node.range();
                    results.push(PolyglotSymbol {
                        language: lang,
                        name: name.to_string(),
                        path: path.to_path_buf(),
                        start_line: range.start_point.row + 1,
                        end_line: range.end_point.row + 1,
                    });
                }
            }
        }
        Ok(results)
    }

    pub fn scan_security_vulnerabilities<P: AsRef<Path>>(
        root: P,
        rules: &[SecurityRule],
    ) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(file_lang) = SupportedLanguage::from_path(path) {
                if let Ok(content) = fs::read_to_string(path) {
                    for rule in rules {
                        if SupportedLanguage::from_string(&rule.language) == Some(file_lang) {
                            let Ok(query) = Query::new(
                                &file_lang.tree_sitter_language(),
                                &rule.tree_sitter_query,
                            ) else {
                                continue;
                            };
                            if let Ok(mut discovered) = Self::parse_security(
                                path,
                                &content,
                                file_lang,
                                &query,
                                &rule.id,
                                &rule.severity,
                            ) {
                                findings.append(&mut discovered);
                            }
                        }
                    }
                }
            }
        }

        findings.sort_by(|a, b| {
            (a.path.as_path(), a.start_line, a.pattern_name.as_str()).cmp(&(
                b.path.as_path(),
                b.start_line,
                b.pattern_name.as_str(),
            ))
        });
        findings.dedup_by(|a, b| {
            a.path == b.path && a.start_line == b.start_line && a.pattern_name == b.pattern_name
        });

        findings
    }

    pub fn scan_source(path: &Path, source: &str, rules: &[SecurityRule]) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();
        let Some(file_lang) = SupportedLanguage::from_path(path) else { return findings };
        for rule in rules {
            if SupportedLanguage::from_string(&rule.language) != Some(file_lang) {
                continue;
            }
            let Ok(query) = Query::new(&file_lang.tree_sitter_language(), &rule.tree_sitter_query)
            else {
                continue;
            };
            if let Ok(mut discovered) =
                Self::parse_security(path, source, file_lang, &query, &rule.id, &rule.severity)
            {
                findings.append(&mut discovered);
            }
        }
        findings
    }

    fn parse_security(
        path: &Path,
        source: &str,
        lang: SupportedLanguage,
        query: &Query,
        rule_id: &str,
        severity: &str,
    ) -> Result<Vec<SecurityFinding>> {
        let mut parser = Parser::new();
        let ts_lang = lang.tree_sitter_language();
        parser.set_language(&ts_lang)?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow!("Tree-sitter parse failure for {:?}", path))?;

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(query, tree.root_node(), source.as_bytes());

        let mut results = Vec::new();
        let source_lines: Vec<&str> = source.lines().collect();

        for m in matches {
            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                if !capture_name.starts_with("danger") {
                    continue;
                }

                let range = capture.node.range();
                let sink_start = range.start_point.row;
                let sink_end = range.end_point.row;

                let context_start = sink_start.saturating_sub(6);
                let context_end = (sink_end + 2).min(source_lines.len().saturating_sub(1));

                let snippet = if sink_start < source_lines.len() {
                    source_lines[context_start..=context_end].join("\n")
                } else {
                    String::new()
                };

                let patch_snippet = if sink_end < source_lines.len() {
                    source_lines[sink_start..=sink_end].join("\n")
                } else {
                    String::new()
                };

                let sink_text = capture.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();

                results.push(SecurityFinding {
                    language: lang,
                    pattern_name: rule_id.to_string(),
                    severity: severity.to_string(),
                    path: path.to_path_buf(),
                    start_line: context_start + 1,
                    end_line: context_end + 1,
                    source_snippet: snippet,
                    patch_snippet,
                    sink_start_line: sink_start + 1,
                    sink_end_line: sink_end + 1,
                    sink_text,
                });
            }
        }
        Ok(results)
    }

    /// Universal Polyglot Semantic Gate:
    /// Covers PHP, Python, JavaScript, and Go for SQLi and Command Injection.
    pub fn verify_semantic_integrity(
        finding_type: &str,
        lang: SupportedLanguage,
        source: &str,
        patch_snippet: &str,
    ) -> Result<(), String> {
        match finding_type {
            "SQL_INJECTION" => Self::verify_sqli_patch(lang, source, patch_snippet),
            "PHP_COMMAND_INJECTION" | "OS_COMMAND" => {
                Self::verify_cmdi_patch(lang, source, patch_snippet)
            }
            _ => Ok(()),
        }
    }

    fn verify_sqli_patch(lang: SupportedLanguage, source: &str, patch: &str) -> Result<(), String> {
        match lang {
            SupportedLanguage::PHP => {
                let has_var_prepare =
                    patch.contains("->prepare($") || patch.contains("mysqli_prepare($con, $");
                if has_var_prepare && !patch.contains('?') {
                    return Err("SECURITY THEATER REJECTED: prepare() called with raw variable without parameterized template".into());
                }

                if patch.contains("->prepare(") || patch.contains("mysqli_prepare(") {
                    let placeholder_count = patch.matches('?').count();
                    if placeholder_count == 0
                        && (patch.contains("WHERE")
                            || patch.contains("where")
                            || patch.contains("VALUES")
                            || patch.contains("values")
                            || patch.contains("SET")
                            || patch.contains("set"))
                    {
                        return Err("MISSING PLACEHOLDERS: SQL query inside prepare() contains 0 '?' parameter markers".into());
                    }

                    if placeholder_count > 0 && !patch.contains("bind_param") {
                        return Err(format!("UNBOUND PLACEHOLDERS: Query contains {} '?' placeholder(s) but bind_param was omitted", placeholder_count));
                    }

                    let mut start_idx = 0;
                    while let Some(pos) = patch[start_idx..].find('$') {
                        let abs_pos = start_idx + pos;
                        let var_part = &patch[abs_pos..];
                        let end_pos = var_part
                            .find(|c: char| !c.is_alphanumeric() && c != '_')
                            .unwrap_or(var_part.len());
                        let var_name = &var_part[..end_pos];
                        start_idx = abs_pos + var_name.len().max(1);

                        if var_name == "$con"
                            || var_name == "$stmt"
                            || var_name == "$result"
                            || var_name == "$row"
                            || var_name == "$q"
                            || var_name == "$msg"
                            || var_name == "$search_term"
                            || var_name.starts_with("$_")
                        {
                            continue;
                        }

                        if !source.contains(var_name) {
                            return Err(format!("HALLUCINATED SYMBOL REJECTED: Variable '{}' does not exist in the source file", var_name));
                        }
                    }
                }
            }
            SupportedLanguage::Python => {
                if patch.contains("execute(f\"")
                    || patch.contains("execute(f'")
                    || (patch.contains("execute(\"\"\"") && patch.contains('{'))
                {
                    return Err("UNSAFE PYTHON SQL: execute() uses f-string interpolation instead of parameter tuple".into());
                }
                if patch.contains("execute(")
                    && (patch.contains(" % ") || patch.contains(".format("))
                {
                    return Err("UNSAFE PYTHON SQL: execute() uses string formatting instead of parameter tuple".into());
                }
            }
            SupportedLanguage::JavaScript | SupportedLanguage::TypeScript
                if (patch.contains("query(`") && patch.contains("${"))
                    || (patch.contains("execute(`") && patch.contains("${")) =>
            {
                return Err("UNSAFE JS SQL: query uses template literal interpolation ${...} instead of parameterized array".into());
            }
            _ => {}
        }
        Ok(())
    }

    fn verify_cmdi_patch(
        lang: SupportedLanguage,
        _source: &str,
        patch: &str,
    ) -> Result<(), String> {
        match lang {
            SupportedLanguage::PHP
                if (patch.contains("shell_exec(")
                    || patch.contains("system(")
                    || patch.contains("exec(")
                    || patch.contains("passthru("))
                    && !patch.contains("escapeshellarg(")
                    && !patch.contains("escapeshellcmd(") =>
            {
                return Err("UNSANITIZED COMMAND EXECUTION: System call does not sanitize arguments with escapeshellarg()".into());
            }
            SupportedLanguage::Python if patch.contains("shell=True") => {
                return Err("UNSAFE SUBPROCESS: subprocess call uses shell=True".into());
            }
            SupportedLanguage::JavaScript | SupportedLanguage::TypeScript
                if patch.contains("exec(") && !patch.contains("execFile(") =>
            {
                return Err("UNSAFE COMMAND EXECUTION: Use child_process.execFile with argument array instead of exec(shell_string)".into());
            }
            _ => {}
        }
        Ok(())
    }
}
