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

    /// In-memory syntax gate: true iff the source parses to an error-free tree.
    /// Fail-closed for every language (unlike external linters, which may be
    /// absent). Used by auto-heal BEFORE any disk write.
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
    /// Authoritative severity from the rule definition (not the LLM).
    pub severity: String,
    pub path: PathBuf,
    // --- Sink identity (authoritative: use for verification and splice anchoring) ---
    pub sink_start_line: usize, // 1-based, inclusive
    pub sink_end_line: usize,   // 1-based, inclusive
    pub sink_text: String,      // exact source text of the @danger capture
    // --- Context window (display / triage only) ---
    pub start_line: usize, // window start (sink - 20)
    pub end_line: usize,   // window end   (sink + 3)
    pub source_snippet: String, // wide window, for the triage LLM
    /// NARROW snippet: exactly the sink lines. This is the auto-heal
    /// replacement target — the model must never see the wide window.
    pub patch_snippet: String,
}

pub struct PolyglotScanner;

impl PolyglotScanner {
    pub fn load_rules<P: AsRef<Path>>(rules_dir: P) -> Vec<SecurityRule> {
        // WalkDir recurses subdirectories (php/, python/, etc).
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
        let query = Query::new(&ts_lang, lang.symbol_query()).map_err(|e| anyhow!("Failed to compile query: {}", e))?;
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

        // Compile each rule's query ONCE, and fail loudly: a rule with a
        // broken query used to be silently skipped in every file.
        let mut compiled: Vec<(usize, Query)> = Vec::new();
        for (idx, rule) in rules.iter().enumerate() {
            let Some(lang) = SupportedLanguage::from_string(&rule.language) else { continue };
            match Query::new(&lang.tree_sitter_language(), &rule.tree_sitter_query) {
                Ok(q) => compiled.push((idx, q)),
                Err(e) => eprintln!(
                    "==> [Rules Engine] Rule '{}' has an INVALID query and will not run: {}",
                    rule.id, e
                ),
            }
        }

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let Some(file_lang) = SupportedLanguage::from_path(path) else { continue };
            let Ok(content) = fs::read_to_string(path) else { continue };

            for (idx, query) in &compiled {
                let rule = &rules[*idx];
                if SupportedLanguage::from_string(&rule.language) != Some(file_lang) { continue; }
                match Self::parse_security_with(path, &content, file_lang, query, rule) {
                    Ok(mut discovered) => findings.append(&mut discovered),
                    Err(e) => eprintln!("==> [Scanner] Rule '{}' failed on {}: {}", rule.id, path.display(), e),
                }
            }
        }

        // One vulnerability (query construction + execution calls) yields
        // several @danger captures whose 24-line windows overlap — that is
        // ONE finding. Two-sided overlap test; symmetric predicate so
        // dedup_by's argument order is irrelevant.
        findings.sort_by(|a, b| {
            (a.path.as_path(), a.pattern_name.as_str(), a.start_line)
                .cmp(&(b.path.as_path(), b.pattern_name.as_str(), b.start_line))
        });
        findings.dedup_by(|a, b| {
            a.path == b.path
                && a.pattern_name == b.pattern_name
                && a.start_line <= b.end_line
                && b.start_line <= a.end_line
        });
        findings
    }

    /// Scan an in-memory source string with all rules matching the file's
    /// language. Used by auto-heal to verify a patch BEFORE writing to disk.
    pub fn scan_source(path: &Path, source: &str, rules: &[SecurityRule]) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();
        let Some(file_lang) = SupportedLanguage::from_path(path) else { return findings };
        for rule in rules {
            if SupportedLanguage::from_string(&rule.language) != Some(file_lang) { continue; }
            let Ok(query) = Query::new(&file_lang.tree_sitter_language(), &rule.tree_sitter_query) else { continue };
            if let Ok(mut discovered) = Self::parse_security_with(path, source, file_lang, &query, rule) {
                findings.append(&mut discovered);
            }
        }
        findings
    }

    fn parse_security_with(
        path: &Path,
        source: &str,
        lang: SupportedLanguage,
        query: &Query,
        rule: &SecurityRule,
    ) -> Result<Vec<SecurityFinding>> {
        let mut parser = Parser::new();
        parser.set_language(&lang.tree_sitter_language())?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow!("Tree-sitter parse failure for {:?}", path))?;

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        let source_lines: Vec<&str> = source.lines().collect();
        let last_idx = source_lines.len().saturating_sub(1);

        let mut results = Vec::new();
        for m in matches {
            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                if !capture_name.starts_with("danger") { continue; }

                let node = capture.node;
                let range = node.range();
                let sink_start = range.start_point.row; // 0-based, inclusive
                let sink_end = range.end_point.row;
                if sink_start > last_idx { continue; }

                let sink_text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                let context_start = sink_start.saturating_sub(20);
                let context_end = (sink_end + 3).min(last_idx);

                results.push(SecurityFinding {
                    language: lang,
                    pattern_name: rule.id.to_string(),
                    severity: rule.severity.clone(),
                    path: path.to_path_buf(),
                    sink_start_line: sink_start + 1,
                    sink_end_line: sink_end + 1,
                    sink_text,
                    start_line: context_start + 1,
                    end_line: context_end + 1,
                    source_snippet: source_lines[context_start..=context_end].join("\n"),
                    patch_snippet: source_lines[sink_start..=sink_end.min(last_idx)].join("\n"),
                });
            }
        }
        Ok(results)
    }
}
