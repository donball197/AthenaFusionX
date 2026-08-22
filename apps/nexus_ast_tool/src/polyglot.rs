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
    Rust, Zig, Python, JavaScript, TypeScript, C, Cpp, Go, PHP,
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
}

pub struct PolyglotScanner;

impl PolyglotScanner {
    pub fn load_rules<P: AsRef<Path>>(rules_dir: P) -> Vec<SecurityRule> {
        // WalkDir recurses subdirectories (php/, python/, etc). read_dir did not,
        // which silently dropped every rule not sitting at the top level.
        let mut rules = Vec::new();
        for entry in WalkDir::new(rules_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "yaml" || ext == "yml" {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        match serde_yaml::from_str::<SecurityRule>(&content) {
                            Ok(rule) => rules.push(rule),
                            Err(e) => eprintln!("==> [Warning] Failed to parse YAML rule file {}: {}", path.display(), e),
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

    fn parse_symbols(path: &Path, source: &str, lang: SupportedLanguage) -> Result<Vec<PolyglotSymbol>> {
        let mut parser = Parser::new();
        let ts_lang = lang.tree_sitter_language();
        parser.set_language(&ts_lang).map_err(|e| anyhow!("Failed setting parser language: {e}"))?;
        let tree = parser.parse(source, None).ok_or_else(|| anyhow!("Tree-sitter parse failure"))?;
        let query = Query::new(&ts_lang, lang.symbol_query()).map_err(|e| anyhow!("Failed to compile query: {e}"))?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        let mut results = Vec::new();
        for m in matches {
            for capture in m.captures {
                if let Ok(name) = capture.node.utf8_text(source.as_bytes()) {
                    let range = capture.node.range();
                    results.push(PolyglotSymbol {
                        language: lang, name: name.to_string(), path: path.to_path_buf(),
                        start_line: range.start_point.row + 1, end_line: range.end_point.row + 1,
                    });
                }
            }
        }
        Ok(results)
    }

    pub fn scan_security_vulnerabilities<P: AsRef<Path>>(root: P, rules: &[SecurityRule]) -> Vec<SecurityFinding> {
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
    }

    fn parse_security(path: &Path, source: &str, lang: SupportedLanguage, query_str: &str, rule_id: &str) -> Result<Vec<SecurityFinding>> {
        let mut parser = Parser::new();
        let ts_lang = lang.tree_sitter_language();
        parser.set_language(&ts_lang)?;
        let tree = parser.parse(source, None).ok_or_else(|| anyhow!("Tree-sitter parse failure for {:?}", path))?;

        let query = Query::new(&ts_lang, query_str).map_err(|e| anyhow!("Failed to compile security query for {:?}: {}", lang, e))?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        let mut results = Vec::new();
        let source_lines: Vec<&str> = source.lines().collect();

        for m in matches {
            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                if !capture_name.starts_with("danger") { continue; }

                let range = capture.node.range();
                let start_line = range.start_point.row;
                let end_line = range.end_point.row;
                
                let context_start = start_line.saturating_sub(20);
                let context_end = (end_line + 3).min(source_lines.len().saturating_sub(1));
                let snippet = if start_line < source_lines.len() {
                    source_lines[context_start..=context_end].join("\n")
                } else { String::new() };

                // Narrow, exact-line-range snippet used ONLY as the auto-heal
                // replacement target. Must match PolyglotSplicer::splice's
                // start_line..end_line range exactly, or the LLM writes a patch
                // sized to the wide `snippet` above while splice only cuts out
                // this narrow range -- guaranteeing duplicated context and a
                // syntax-check failure on every auto-heal attempt.
                let _patch_target = if end_line < source_lines.len() {
                    source_lines[start_line..=end_line].join("\n")
                } else { String::new() };

                results.push(SecurityFinding {
                    language: lang,
                    pattern_name: rule_id.to_string(), 
                    path: path.to_path_buf(),
                    start_line: context_start + 1,
                    end_line: context_end + 1,
                    source_snippet: snippet.clone(),
                    patch_snippet: snippet,
                });
            }
        }
        results.dedup_by(|a, b| a.start_line == b.start_line && a.pattern_name == b.pattern_name);
        Ok(results)
    }
}
