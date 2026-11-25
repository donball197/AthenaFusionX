//! Polyglot Language Engine & Universal Symbol Discovery
//! Enforcing Zero Fragmentation (Strict 0.23.x ABI)

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Parser, Query, QueryCursor};
use walkdir::WalkDir;
use crate::callgraph::{enclosing_function_name, CallGraphIndex, TaintLocation};
use crate::taint::{TaintAnalyzer, TaintSource, find_enclosing_function};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSitterParseStatus {
    Clean,
    TreeSitterError,
    ParserUnavailable,
    NoParseTree,
}

impl TreeSitterParseStatus {
    pub fn is_clean(self) -> bool {
        matches!(self, Self::Clean)
    }
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "rs" => Some(Self::Rust),
            "zig" => Some(Self::Zig),
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

    /// JavaScript rules apply to TypeScript files and vice versa,
    /// because tree-sitter uses the same node types for both.
    pub fn rule_applies_to(rule_lang: &str, file_lang: SupportedLanguage) -> bool {
        match (Self::from_string(rule_lang), file_lang) {
            (Some(rl), fl) if rl == fl => true,
            (Some(Self::JavaScript), Self::TypeScript) => true,
            (Some(Self::TypeScript), Self::JavaScript) => true,
            _ => false,
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

    pub fn tree_sitter_language_for_path(&self, path: &Path) -> Language {
        if *self == Self::TypeScript
            && path.extension().and_then(|s| s.to_str()) == Some("tsx")
        {
            return tree_sitter_typescript::LANGUAGE_TSX.into();
        }
        self.tree_sitter_language()
    }

    pub fn symbol_query(&self) -> &'static str {
        match self {
            Self::Rust => "(function_item name: (identifier) @name)",
            Self::Zig => "(function_declaration name: (identifier) @name)",
            Self::Python => "(function_definition name: (identifier) @name)",
            Self::JavaScript => {
                r#"
                [
                    (function_declaration
                        name: (identifier) @name
                        body: (statement_block) @body)
                    (method_definition
                        name: (_) @name
                        body: (statement_block) @body)
                    (variable_declarator
                        name: (identifier) @name
                        value: (function_expression
                            body: (statement_block) @body))
                    (variable_declarator
                        name: (identifier) @name
                        value: (arrow_function
                            body: (statement_block) @body))
                ]
            "#
            }
            Self::TypeScript => {
                r#"
                [
                    (function_declaration
                        name: (identifier) @name
                        body: (statement_block) @body)
                    (method_definition
                        name: (_) @name
                        body: (statement_block) @body)
                    (variable_declarator
                        name: (identifier) @name
                        value: (function_expression
                            body: (statement_block) @body))
                    (variable_declarator
                        name: (identifier) @name
                        value: (arrow_function
                            body: (statement_block) @body))
                ]
            "#
            }
            Self::C => "(function_declarator declarator: (identifier) @name)",
            Self::Cpp => "(function_declarator declarator: (identifier) @name)",
            Self::Go => "(function_declaration name: (identifier) @name)",
            Self::PHP => "(function_definition name: (name) @name)",
        }
    }

    pub fn tree_sitter_parse_status(&self, source: &str) -> TreeSitterParseStatus {
        let mut parser = Parser::new();
        let language = self.tree_sitter_language();

        if parser.set_language(&language).is_err() {
            return TreeSitterParseStatus::ParserUnavailable;
        }

        match parser.parse(source, None) {
            Some(tree) if !tree.root_node().has_error() => TreeSitterParseStatus::Clean,
            Some(tree) => {
                fn find_err(node: tree_sitter::Node, src: &str) {
                    if node.is_error() || node.is_missing() {
                        let pos = node.start_position();
                        let text = node.utf8_text(src.as_bytes()).unwrap_or("<utf8>");
                        eprintln!(
                            "==> [AST ERROR] Line {}, Col {}: '{}' (node: {})",
                            pos.row + 1,
                            pos.column + 1,
                            text,
                            node.kind()
                        );
                    }
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        find_err(child, src);
                    }
                }
                find_err(tree.root_node(), source);
                TreeSitterParseStatus::TreeSitterError
            }
            None => TreeSitterParseStatus::NoParseTree,
        }
    }

    pub fn parses_cleanly(&self, source: &str) -> bool {
        self.tree_sitter_parse_status(source).is_clean()
    }

    pub fn parses_cleanly_for_path(&self, path: &Path, source: &str) -> bool {
        let mut parser = Parser::new();
        let language = self.tree_sitter_language_for_path(path);
        if parser.set_language(&language).is_err() {
            return false;
        }
        matches!(parser.parse(source, None), Some(tree) if !tree.root_node().has_error())
    }

    /// Expand a 1-based sink line range to its enclosing AST statement lines (1-based),
    /// preventing partial multi-line call replacements during auto-heal.
    pub fn enclosing_statement_lines(
        &self,
        path: &Path,
        source: &str,
        start_line: usize,
        end_line: usize,
    ) -> Option<(usize, usize)> {
        let mut parser = Parser::new();
        let language = self.tree_sitter_language_for_path(path);
        parser.set_language(&language).ok()?;
        let tree = parser.parse(source, None)?;

        let lines: Vec<&str> = source.lines().collect();
        if start_line == 0 || end_line == 0 || end_line > lines.len() || start_line > end_line {
            return None;
        }
        let start_col = lines[start_line - 1]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end_col = lines[end_line - 1].trim_end().len();

        let mut node = tree.root_node().descendant_for_point_range(
            tree_sitter::Point::new(start_line - 1, start_col),
            tree_sitter::Point::new(end_line - 1, end_col),
        )?;

        while let Some(parent) = node.parent() {
            if matches!(
                parent.kind(),
                "module"
                    | "block"
                    | "statement_block"
                    | "compound_statement"
                    | "program"
                    | "source_file"
                    | "translation_unit"
                    | "class_body"
            ) {
                break;
            }
            if matches!(
                node.kind(),
                "expression_statement"
                    | "assignment"
                    | "return_statement"
                    | "lexical_declaration"
                    | "variable_declaration"
                    | "short_var_declaration"
                    | "assignment_statement"
                    | "let_declaration"
                    | "declaration"
            ) {
                break;
            }
            node = parent;
        }

        let s = node.start_position().row + 1;
        let e = node.end_position().row + 1;
        if s <= start_line && e >= end_line && e.saturating_sub(s) <= 25 {
            Some((s, e))
        } else {
            None
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

#[derive(Debug, Clone, serde::Serialize)]
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
    pub taint_path: Vec<String>,
    pub taint_locations: Vec<TaintLocation>,
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

fn is_ignored(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git")
                | Some("target")
                | Some("node_modules")
                | Some(".venv")
                | Some("venv")
                | Some("__pycache__")
                | Some("site-packages")
                | Some("dist-info")
                | Some("egg-info")
        )
    })
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
                            Ok(rule) => {
                                match SupportedLanguage::from_string(&rule.language) {
                                    Some(lang) => {
                                        if let Err(e) = Query::new(
                                            &lang.tree_sitter_language(),
                                            &rule.tree_sitter_query,
                                        ) {
                                            eprintln!(
                                                "==> [RULE DISABLED] {} ({}): tree-sitter query does not compile -- this rule will NEVER produce findings until fixed.\n    File: {}\n    Error: {}",
                                                rule.id,
                                                rule.language,
                                                path.display(),
                                                e.to_string().lines().next().unwrap_or("")
                                            );
                                            continue;
                                        }
                                    }
                                    None => {
                                        eprintln!(
                                            "==> [RULE DISABLED] {}: unrecognized language string '{}' in {}",
                                            rule.id,
                                            rule.language,
                                            path.display()
                                        );
                                        continue;
                                    }
                                }
                                rules.push(rule)
                            }
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
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| !is_ignored(e.path()) && (include_tests() || e.depth() == 0 || !is_test_noise(e)))
            .filter_map(|e| e.ok())
        {
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
        let all_langs = [
            SupportedLanguage::Rust,
            SupportedLanguage::Zig,
            SupportedLanguage::Python,
            SupportedLanguage::JavaScript,
            SupportedLanguage::TypeScript,
            SupportedLanguage::C,
            SupportedLanguage::Cpp,
            SupportedLanguage::Go,
            SupportedLanguage::PHP,
        ];
        let compiled_rules: Vec<(&SecurityRule, SupportedLanguage, Query)> = rules
            .iter()
            .flat_map(|rule| {
                all_langs
                    .into_iter()
                    .filter(|&lang| SupportedLanguage::rule_applies_to(&rule.language, lang))
                    .filter_map(move |lang| {
                        Query::new(&lang.tree_sitter_language(), &rule.tree_sitter_query)
                            .ok()
                            .map(|q| (rule, lang, q))
                    })
            })
            .collect();

        let mut loaded_files: Vec<(PathBuf, String, SupportedLanguage)> = Vec::new();
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| !is_ignored(e.path()) && (include_tests() || e.depth() == 0 || !is_test_noise(e)))
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Some(file_lang) = SupportedLanguage::from_path(path) {
                if let Ok(content) = fs::read_to_string(path) {
                    loaded_files.push((path.to_path_buf(), content, file_lang));
                }
            }
        }

        let call_graph = CallGraphIndex::build(&loaded_files);

        for (path, content, file_lang) in &loaded_files {
            let is_tsx = *file_lang == SupportedLanguage::TypeScript
                && path.extension().and_then(|s| s.to_str()) == Some("tsx");
            for (rule, rule_lang, query) in &compiled_rules {
                if *rule_lang == *file_lang {
                    let tsx_query;
                    let active_query = if is_tsx {
                        let Ok(q) = Query::new(
                            &file_lang.tree_sitter_language_for_path(path),
                            &rule.tree_sitter_query,
                        ) else {
                            continue;
                        };
                        tsx_query = q;
                        &tsx_query
                    } else {
                        query
                    };
                    if let Ok(mut discovered) = Self::parse_security_with_callgraph(
                        path,
                        content,
                        *file_lang,
                        active_query,
                        &rule.id,
                        &rule.severity,
                        Some(&call_graph),
                    ) {
                        findings.append(&mut discovered);
                    }
                }
            }
        }

        findings.sort_by(|a, b| {
            (a.path.as_path(), a.sink_start_line, a.sink_end_line, a.pattern_name.as_str()).cmp(&(
                b.path.as_path(),
                b.sink_start_line,
                b.sink_end_line,
                b.pattern_name.as_str(),
            ))
        });
        findings.dedup_by(|a, b| {
            a.path == b.path
                && a.sink_start_line == b.sink_start_line
                && a.sink_end_line == b.sink_end_line
                && a.pattern_name == b.pattern_name
        });

        findings
    }

    pub fn scan_source(path: &Path, source: &str, rules: &[SecurityRule]) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();
        let Some(file_lang) = SupportedLanguage::from_path(path) else { return findings };
        let ts_lang = file_lang.tree_sitter_language_for_path(path);
        for rule in rules {
            if !SupportedLanguage::rule_applies_to(&rule.language, file_lang) {
                continue;
            }
            let Ok(query) = Query::new(&ts_lang, &rule.tree_sitter_query)
            else {
                continue;
            };
            if let Ok(mut discovered) =
                Self::parse_security(path, source, file_lang, &query, &rule.id, &rule.severity)
            {
                findings.append(&mut discovered);
            }
        }
        findings.sort_by(|a, b| {
            (a.path.as_path(), a.sink_start_line, a.sink_end_line, a.pattern_name.as_str()).cmp(&(
                b.path.as_path(),
                b.sink_start_line,
                b.sink_end_line,
                b.pattern_name.as_str(),
            ))
        });
        findings.dedup_by(|a, b| {
            a.path == b.path
                && a.sink_start_line == b.sink_start_line
                && a.sink_end_line == b.sink_end_line
                && a.pattern_name == b.pattern_name
        });
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
        Self::parse_security_with_callgraph(path, source, lang, query, rule_id, severity, None)
    }

    fn parse_security_with_callgraph(
        path: &Path,
        source: &str,
        lang: SupportedLanguage,
        query: &Query,
        rule_id: &str,
        severity: &str,
        call_graph: Option<&CallGraphIndex>,
    ) -> Result<Vec<SecurityFinding>> {
        let mut parser = Parser::new();
        let ts_lang = lang.tree_sitter_language_for_path(path);
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

                // Check for inline suppression comment on the sink line or preceding line
                let is_suppressed = if sink_start < source_lines.len() {
                    let check_start = sink_start.saturating_sub(1);
                    source_lines[check_start..=sink_start].iter().any(|line| {
                        (line.contains("nexus-ignore")
                            || line.contains("nosemgrep")
                            || line.contains("#[allow("))
                            && (line.contains(rule_id) || line.contains("all"))
                    })
                } else {
                    false
                };

                if is_suppressed {
                    continue;
                }

                // Avoid false positives on bare json.loads() when matching `from pickle import loads`
                if rule_id == "PYTHON_INSECURE_DESERIALIZATION" {
                    if let Some(call_node) = capture.node.parent() {
                        if let Some(func_node) = call_node.child_by_field_name("function") {
                            if func_node.kind() == "identifier" {
                                let has_unsafe_import = source.contains("pickle")
                                    || source.contains("cPickle")
                                    || source.contains("marshal");
                                if !has_unsafe_import {
                                    continue;
                                }
                            }
                        }
                    }
                }

                let mut taint_path = Vec::new();
                let mut taint_locations = Vec::new();

                if matches!(
                    rule_id,
                    "SQL_INJECTION"
                        | "GO_SQL_INJECTION"
                        | "RUST_SQL_INJECTION"
                        | "COMMAND_INJECTION"
                        | "GO_COMMAND_INJECTION"
                        | "PHP_COMMAND_INJECTION"
                        | "RUST_COMMAND_INJECTION"
                        | "C_COMMAND_INJECTION"
                        | "CPP_COMMAND_INJECTION"
                        | "JS_XSS"
                        | "PYTHON_INSECURE_DESERIALIZATION"
                        | "PYTHON_SSTI"
                ) {
                    let func_node = find_enclosing_function(capture.node);
                    let mut analyzer = TaintAnalyzer::new(source, lang);
                    if let Some(func) = func_node {
                        let (has_callers, caller_seeds) = match (
                            call_graph,
                            enclosing_function_name(func, source),
                        ) {
                            (Some(cg), Some(ref fname)) => cg.seeds_for(fname),
                            _ => (false, Vec::new()),
                        };
                        let simple_seeds: Vec<(usize, TaintSource, Vec<String>)> = caller_seeds
                            .iter()
                            .map(|s| (s.param_index, s.source, s.trace.clone()))
                            .collect();
                        analyzer.analyze_function_with_callers(
                            func,
                            &simple_seeds,
                            has_callers,
                            sink_start,
                        );
                        for seed in caller_seeds {
                            if !taint_locations.contains(&seed.caller_step) {
                                taint_locations.push(seed.caller_step);
                            }
                        }
                    }
                    let taint_report = analyzer.check_expression(capture.node);
                    if !taint_report.tainted {
                        continue;
                    }
                    taint_path = taint_report.path;
                    if taint_locations.is_empty() && !taint_path.is_empty() {
                        let source_line = func_node
                            .map(|f| f.start_position().row + 1)
                            .unwrap_or(sink_start + 1);
                        taint_locations.push(TaintLocation {
                            path: path.to_path_buf(),
                            line: source_line,
                            message: format!("Taint origin: {}", taint_path.join(" -> ")),
                        });
                    }
                }

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
                    taint_path,
                    taint_locations,
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
            "SQL_INJECTION" | "GO_SQL_INJECTION" => {
                Self::verify_sqli_patch(lang, source, patch_snippet)
            }
            "PHP_COMMAND_INJECTION"
            | "OS_COMMAND"
            | "PYTHON_OS_COMMAND_INJECTION"
            | "COMMAND_INJECTION"
            | "GO_COMMAND_INJECTION" => {
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
                        return Err(format!(
                            "UNBOUND PLACEHOLDERS: Query contains {} '?' placeholder(s) but bind_param was omitted",
                            placeholder_count
                        ));
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
                            return Err(format!(
                                "HALLUCINATED SYMBOL REJECTED: Variable '{}' does not exist in the source file",
                                var_name
                            ));
                        }
                    }
                }
            }
            SupportedLanguage::Python => {
                if patch.contains("execute(f\"")
                    || patch.contains("execute(f'")
                    || patch.contains("text(f\"")
                    || patch.contains("text(f'")
                    || ((patch.contains("execute(\"\"\"") || patch.contains("text(\"\"\""))
                        && patch.contains('{'))
                {
                    return Err("UNSAFE PYTHON SQL: query uses f-string interpolation instead of bound parameters".into());
                }
                if (patch.contains("execute(") || patch.contains("text("))
                    && (patch.contains(" % ") || patch.contains(".format("))
                {
                    return Err("UNSAFE PYTHON SQL: query uses string formatting instead of bound parameters".into());
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
            SupportedLanguage::Go => {
                let has_allowlist = patch.contains("map[string]bool{")
                    || patch.contains(":= map[")
                    || patch.contains("allowed :=")
                    || patch.contains("allowed =")
                    || patch.contains("allowlist :=")
                    || patch.contains("allowlist =")
                    || patch.contains("switch ");
                if !has_allowlist {
                    for call in ["exec.Command(", "exec.CommandContext("] {
                        if let Some(pos) = patch.find(call) {
                            let after = &patch[pos + call.len()..];
                            let trimmed = after.trim_start();
                            let arg_start = if call.contains("CommandContext") {
                                match trimmed.find(',') {
                                    Some(c) => trimmed[c + 1..].trim_start(),
                                    None => continue,
                                }
                            } else {
                                trimmed
                            };
                            if arg_start
                                .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                            {
                                if let Some(close) = arg_start.find(')') {
                                    let arg = &arg_start[..close];
                                    if !arg.contains(',') && !arg.contains('"') {
                                        return Err(format!(
                                            "unsafe: user-controlled variable `{}` passed directly to {} without allowlist",
                                            arg.trim(),
                                            call.trim_end_matches('('),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}


// ---- nexus_tune additions ----
fn include_tests() -> bool {
    std::env::var("NEXUS_INCLUDE_TESTS").is_ok()
}

fn is_test_noise(e: &walkdir::DirEntry) -> bool {
    let name = e.file_name().to_str().unwrap_or("");
    if e.file_type().is_dir() {
        return matches!(name, "test" | "tests" | "__tests__" | "cypress" | "e2e" | "spec" | "mocks");
    }
    const SUFFIXES: &[&str] = &[
        ".spec.ts", ".test.ts", ".spec.js", ".test.js",
        ".config.ts", ".config.js", ".d.ts", ".min.js",
    ];
    SUFFIXES.iter().any(|s| name.ends_with(s))
}

