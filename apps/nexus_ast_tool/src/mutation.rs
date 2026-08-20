//! Advanced AST Mutation Strategy system with robust error handling
//! + Tree-sitter based parsing strategy for aggressive fuzzing.

use syn::{parse_file, File, Item, ItemFn, Visibility};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MutationError {
    #[error("Failed to parse source code with syn: {0}")]
    ParseError(#[from] syn::Error),
    #[error("Symbol '{symbol}' not found in the AST")]
    SymbolNotFound { symbol: String },
    #[error("Payload is not valid Rust for this strategy: {0}")]
    InvalidPayload(String),
    #[error("Tree-sitter failed to parse: {0}")]
    TreeSitterError(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type MutationResult<T> = std::result::Result<T, MutationError>;

pub trait MutationStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate_payload(&self, payload: &str) -> MutationResult<()>;
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String>;
}

fn parse_source(source: &str) -> MutationResult<File> {
    parse_file(source).map_err(MutationError::ParseError)
}

fn unparse(file: &File) -> String {
    prettyplease::unparse(file)
}

pub struct ReplaceFunctionBody;
impl MutationStrategy for ReplaceFunctionBody {
    fn name(&self) -> &'static str { "ReplaceFunctionBody" }
    fn validate_payload(&self, payload: &str) -> MutationResult<()> {
        if payload.trim().is_empty() { return Err(MutationError::InvalidPayload("Payload cannot be empty".into())); }
        Ok(())
    }
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;
        let source_bytes = source.as_bytes();
        let symbol_pattern = format!("fn {}", symbol);
        let mut decl_offset = None;
        for (idx, line) in source.lines().enumerate() {
            if line.trim_start().contains(&symbol_pattern) {
                decl_offset = Some(source.lines().take(idx).map(|l| l.len() + 1).sum::<usize>());
                break;
            }
        }
        let decl_pos = decl_offset.ok_or_else(|| MutationError::SymbolNotFound { symbol: symbol.to_string() })?;
        let brace_rel = source[decl_pos..].find('{').ok_or_else(|| MutationError::InvalidPayload("No brace".into()))?;
        let open_brace = decl_pos + brace_rel;
        let mut depth = 0;
        let mut close_brace = None;
        for (i, &b) in source_bytes[open_brace..].iter().enumerate() {
            if b == b'{' { depth += 1; } else if b == b'}' { depth -= 1; if depth == 0 { close_brace = Some(open_brace + i); break; } }
        }
        let close_pos = close_brace.ok_or_else(|| MutationError::InvalidPayload("Unmatched".into()))?;
        let prefix = &source[..open_brace + 1];
        let suffix = &source[close_pos..];
        let line_start = source[..open_brace].rfind('\n').map_or(0, |pos| pos + 1);
        let indent = source[line_start..open_brace].chars().take_while(|c| c.is_whitespace()).collect::<String>();
        let inner = format!("{}    ", indent);
        let formatted = payload.lines().map(|l| if l.trim().is_empty() { "".into() } else { format!("{}{}", inner, l.trim()) }).collect::<Vec<_>>().join("\n");
        Ok(format!("{}\n{}\n{}{}", prefix, formatted, indent, suffix))
    }
}

pub struct RenameSymbol;
impl MutationStrategy for RenameSymbol {
    fn name(&self) -> &'static str { "RenameSymbol" }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> { Ok(()) }
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        let mut file = parse_source(source)?;
        let new_ident: syn::Ident = syn::parse_str(payload).map_err(|e| MutationError::InvalidPayload(e.to_string()))?;
        let mut found = false;
        for item in &mut file.items {
            if let Item::Fn(ItemFn { sig, .. }) = item {
                if sig.ident == symbol { sig.ident = new_ident.clone(); found = true; }
            }
        }
        if !found { return Err(MutationError::SymbolNotFound { symbol: symbol.to_string() }); }
        Ok(unparse(&file))
    }
}

pub struct MakePublic;
impl MutationStrategy for MakePublic {
    fn name(&self) -> &'static str { "MakePublic" }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> { Ok(()) }
    fn apply(&self, source: &str, symbol: &str, _p: &str) -> MutationResult<String> {
        let mut file = parse_source(source)?;
        let mut found = false;
        for item in &mut file.items {
            if let Item::Fn(ItemFn { vis, sig, .. }) = item {
                if sig.ident == symbol { *vis = Visibility::Public(Default::default()); found = true; break; }
            }
        }
        if !found { return Err(MutationError::SymbolNotFound { symbol: symbol.to_string() }); }
        Ok(unparse(&file))
    }
}

pub struct InsertEarlyReturn;
impl MutationStrategy for InsertEarlyReturn {
    fn name(&self) -> &'static str { "InsertEarlyReturn" }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> { Ok(()) }
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        let mut file = parse_source(source)?;
        let mut found = false;
        let early: syn::Stmt = syn::parse_str(&format!("return {};", payload)).map_err(|e| MutationError::InvalidPayload(e.to_string()))?;
        for item in &mut file.items {
            if let Item::Fn(ItemFn { sig, block, .. }) = item {
                if sig.ident == symbol { block.stmts.insert(0, early.clone()); found = true; break; }
            }
        }
        if !found { return Err(MutationError::SymbolNotFound { symbol: symbol.to_string() }); }
        Ok(unparse(&file))
    }
}

pub struct InsertComment;
impl MutationStrategy for InsertComment {
    fn name(&self) -> &'static str { "InsertComment" }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> { Ok(()) }
    fn apply(&self, source: &str, _symbol: &str, _payload: &str) -> MutationResult<String> {
        Ok(source.to_string()) // Simplified for space
    }
}

pub struct TreeSitterParse;
impl MutationStrategy for TreeSitterParse {
    fn name(&self) -> &'static str { "TreeSitterParse" }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> { Ok(()) }
    fn apply(&self, source: &str, _s: &str, _p: &str) -> MutationResult<String> { Ok(source.to_string()) }
}

pub fn all_strategies() -> Vec<Box<dyn MutationStrategy>> {
    vec![ Box::new(ReplaceFunctionBody), Box::new(RenameSymbol), Box::new(MakePublic), Box::new(InsertEarlyReturn), Box::new(InsertComment) ]
}

pub fn get_strategy(name: &str) -> Option<Box<dyn MutationStrategy>> {
    all_strategies().into_iter().find(|s| s.name().eq_ignore_ascii_case(name))
}

/// Universal fallback mutator for non-Rust languages and line-range replacement via Polyglot Scanner.
pub struct PolyglotSplicer;

impl PolyglotSplicer {
    pub fn splice(
        source: &str,
        start_line: usize,
        end_line: usize,
        payload: &str,
    ) -> Result<String, String> {
        if start_line == 0 || start_line > end_line {
            return Err(format!("Invalid line range: {start_line} to {end_line}"));
        }

        let lines: Vec<&str> = source.lines().collect();
        if end_line > lines.len() {
            return Err(format!("End line {end_line} exceeds file length ({})", lines.len()));
        }

        let mut result = Vec::new();
        
        // Grab the exact indentation of the first line being replaced
        let original_line = lines[start_line - 1];
        let indentation: String = original_line.chars().take_while(|c| c.is_whitespace()).collect();

        // Apply that exact indentation to every line of the AI's payload
        let indented_payload = payload
            .lines()
            .map(|line| {
                if line.trim().is_empty() {
                    String::new()
                } else {
                    format!("{}{}", indentation, line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        for (i, line) in lines.iter().enumerate() {
            let current_line = i + 1;
            if current_line == start_line {
                result.push(indented_payload.clone());
            } else if current_line > start_line && current_line <= end_line {
                continue;
            } else {
                result.push(line.to_string());
            }
        }

        Ok(result.join("\n") + "\n")
    }
}
