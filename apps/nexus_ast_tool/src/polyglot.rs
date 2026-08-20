//! Polyglot Language Engine & Universal Symbol Discovery
//! Enforcing Zero Fragmentation (Strict 0.23.x ABI)
//! Supported: Rust, Zig, Python, TS, JS, C, C++, Go, Svelte.

use anyhow::{anyhow, Result};
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
    Svelte,
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
            "svelte" => Some(Self::Svelte),
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
            Self::Svelte => tree_sitter_svelte_ng::LANGUAGE.into(),
        }
    }

    pub fn symbol_query(&self) -> &'static str {
        match self {
            Self::Rust => "(function_item name: (identifier) @name)",
            Self::Zig => "(FnProto (IDENTIFIER) @name)",
            Self::Python => "(function_definition name: (identifier) @name)",
            Self::JavaScript => "(function_declaration name: (identifier) @name)",
            Self::TypeScript => "(function_declaration name: (identifier) @name)",
            Self::C => "(function_declarator declarator: (identifier) @name)",
            Self::Cpp => "(function_declarator declarator: (identifier) @name)",
            Self::Go => "(function_declaration name: (identifier) @name)",
            // Svelte components: targeting script blocks or elements
            Self::Svelte => "(script_element) @name", 
        }
    }
}

#[derive(Debug, Clone)]
pub struct PolyglotSymbol {
    pub language: SupportedLanguage,
    pub name: String,
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

pub struct PolyglotScanner;

impl PolyglotScanner {
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
        parser
            .set_language(&ts_lang)
            .map_err(|e| anyhow!("Failed setting parser language: {e}"))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow!("Tree-sitter parse failure for {:?}", path))?;

        let query = Query::new(&ts_lang, lang.symbol_query())
            .map_err(|e| anyhow!("Failed to compile query: {e}"))?;

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        let mut results = Vec::new();
        for m in matches {
            for capture in m.captures {
                // Svelte script tags might not have a clean "name" node, 
                // so we fallback to a generic identifier for the AST map
                let mut name = capture.node.utf8_text(source.as_bytes()).unwrap_or("<svelte_block>").to_string();
                if lang == SupportedLanguage::Svelte {
                    name = "<script_block>".to_string();
                }

                let range = capture.node.range();
                results.push(PolyglotSymbol {
                    language: lang,
                    name,
                    path: path.to_path_buf(),
                    start_line: range.start_point.row + 1,
                    end_line: range.end_point.row + 1,
                });
            }
        }
        Ok(results)
    }
}
