//! Core Intelligence Engine for Autonomous Code Healing:
//! 1. AST Codebase Scanner
//! 2. Cargo Error Diagnostics Ingestion
//! 3. Policy & Strategy Selection Engine

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::Visit;
use syn::{parse_file, ItemFn, Visibility};
use walkdir::WalkDir;

// ------------------------------------------------------------------
// 1. Codebase Scanner (The Senses)
// ------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SymbolLocation {
    pub name: String,
    pub path: PathBuf,
    pub is_public: bool,
}

struct AstScanner {
    path: PathBuf,
    symbols: Vec<SymbolLocation>,
}

impl<'ast> Visit<'ast> for AstScanner {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let is_public = matches!(node.vis, Visibility::Public(_));
        self.symbols.push(SymbolLocation {
            name: node.sig.ident.to_string(),
            path: self.path.clone(),
            is_public,
        });
        syn::visit::visit_item_fn(self, node);
    }
}

pub fn scan_directory<P: AsRef<Path>>(root: P) -> Vec<SymbolLocation> {
    let mut symbols = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().is_some_and(|ext| ext == "rust" || ext == "rs") {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(file) = parse_file(&content) {
                    let mut visitor = AstScanner {
                        path: entry.path().to_path_buf(),
                        symbols: Vec::new(),
                    };
                    visitor.visit_file(&file);
                    symbols.extend(visitor.symbols);
                }
            }
        }
    }
    symbols
}

// ------------------------------------------------------------------
// 2. Diagnostics Ingestion (The Trigger)
// ------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct CompilerDiagnosticMessage {
    pub message: String,
    pub code: Option<DiagnosticCode>,
    pub spans: Vec<DiagnosticSpan>,
}

#[derive(Deserialize, Debug)]
pub struct DiagnosticCode {
    pub code: String,
}

#[derive(Deserialize, Debug)]
pub struct DiagnosticSpan {
    pub file_name: String,
    pub line_start: usize,
    pub text: Vec<DiagnosticSpanText>,
}

#[derive(Deserialize, Debug)]
pub struct DiagnosticSpanText {
    pub text: String,
}

pub fn parse_compiler_line(line: &str) -> Option<CompilerDiagnosticMessage> {
    #[derive(Deserialize)]
    struct CargoMessage {
        reason: String,
        message: Option<CompilerDiagnosticMessage>,
    }
    if let Ok(msg) = serde_json::from_str::<CargoMessage>(line) {
        if msg.reason == "compiler-message" {
            return msg.message;
        }
    }
    None
}

// ------------------------------------------------------------------
// 3. Policy & Strategy Selection Engine (The Logic)
// ------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RefactorAction {
    pub file: String,
    pub strategy: String,
    pub symbol: String,
    pub payload: String,
}

pub fn determine_policy(diagnostic: &CompilerDiagnosticMessage) -> Option<RefactorAction> {
    let raw_msg = &diagnostic.message;
    let span = diagnostic.spans.first()?;
    let file = span.file_name.clone();

    // Policy Rule 1: Private item visibility issue
    if raw_msg.contains("is private") {
        if let Some(symbol) = extract_symbol_name(raw_msg) {
            return Some(RefactorAction {
                file,
                strategy: "MakePublic".into(),
                symbol,
                payload: "".into(),
            });
        }
    }

    // Policy Rule 2: Unused/Dead code warning or error stubbing
    if raw_msg.contains("unused variable") || raw_msg.contains("unreachable") {
        if let Some(symbol) = extract_symbol_name(raw_msg) {
            return Some(RefactorAction {
                file,
                strategy: "InsertEarlyReturn".into(),
                symbol,
                payload: "()".into(),
            });
        }
    }

    None
}

fn extract_symbol_name(msg: &str) -> Option<String> {
    let start = msg.find('`')? + 1;
    let end = msg[start..].find('`')? + start;
    Some(msg[start..end].to_string())
}
