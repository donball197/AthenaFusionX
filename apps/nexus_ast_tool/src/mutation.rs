// apps/nexus_ast_tool/src/mutation.rs

//! Advanced AST Mutation Strategy system with robust error handling.
//! Uses syn for safe semantic boundary detection, but preserves comments
//! by slicing the original source string rather than unparsing the AST.

use proc_macro2::LineColumn;
use syn::spanned::Spanned;
use syn::{parse_file, Item, ItemFn, Visibility};
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

/// Helper function to convert proc_macro2::LineColumn to a byte offset in a string.
pub(crate) fn line_column_to_byte_offset(source: &str, loc: LineColumn) -> usize {
    let mut current_line = 1;
    let mut current_col = 0;
    for (i, ch) in source.char_indices() {
        if current_line == loc.line && current_col == loc.column {
            return i;
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 0;
        } else {
            current_col += 1;
        }
    }
    source.len()
}

/// Helper to find a function block span in top-level items or impl blocks.
fn find_function_block_span(file: &syn::File, symbol: &str) -> MutationResult<proc_macro2::Span> {
    file.items
        .iter()
        .find_map(|item| match item {
            Item::Fn(ItemFn { sig, block, .. }) if sig.ident == symbol => Some(block.span()),
            Item::Impl(syn::ItemImpl { items, .. }) => items.iter().find_map(|i| {
                if let syn::ImplItem::Fn(method) = i {
                    if method.sig.ident == symbol {
                        return Some(method.block.span());
                    }
                }
                None
            }),
            _ => None,
        })
        .ok_or_else(|| MutationError::SymbolNotFound { symbol: symbol.to_string() })
}

/// Helper to format and replace a function body block while preserving indentation.
fn replace_block_body(
    source: &str,
    target_span: proc_macro2::Span,
    payload: &str,
) -> MutationResult<String> {
    let start = line_column_to_byte_offset(source, target_span.start());
    let end = line_column_to_byte_offset(source, target_span.end());

    let block_str = &source[start..end];
    let inner_start_rel = block_str.find('{').map_or(0, |p| p + 1);
    let inner_end_rel = block_str.rfind('}').unwrap_or(block_str.len());

    let abs_inner_start = start + inner_start_rel;
    let abs_inner_end = start + inner_end_rel;

    let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
    let indent =
        source[line_start..start].chars().take_while(|c| c.is_whitespace()).collect::<String>();

    let inner_indent = format!("{}    ", indent);
    let formatted_body = payload
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                "".to_string()
            } else {
                format!("{}{}", inner_indent, l.trim())
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let new_block_str = format!("\n{}\n{}", formatted_body, indent);

    let mut new_source = source.to_string();
    new_source.replace_range(abs_inner_start..abs_inner_end, &new_block_str);
    Ok(new_source)
}

pub struct ReplaceFunctionBody;
impl MutationStrategy for ReplaceFunctionBody {
    fn name(&self) -> &'static str {
        "ReplaceFunctionBody"
    }
    fn validate_payload(&self, payload: &str) -> MutationResult<()> {
        if payload.trim().is_empty() {
            return Err(MutationError::InvalidPayload("Payload cannot be empty".into()));
        }
        syn::parse_str::<syn::Block>(&format!("{{ {} }}", payload))
            .map_err(|e| MutationError::InvalidPayload(format!("Invalid Rust block: {}", e)))?;
        Ok(())
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;
        let file = parse_file(source)?;
        let target_fn = find_function_block_span(&file, symbol)?;
        replace_block_body(source, target_fn, payload)
    }
}

pub struct RenameSymbol;
impl MutationStrategy for RenameSymbol {
    fn name(&self) -> &'static str {
        "RenameSymbol"
    }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> {
        Ok(())
    }
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;
        if payload.trim().is_empty() {
            return Err(MutationError::InvalidPayload("New symbol name cannot be empty".into()));
        }
        // Simple string/identifier replacement for the symbol across the source
        // A robust implementation would use AST rewriter, but simple replacement works for targeted renames.
        if !source.contains(symbol) {
            return Err(MutationError::SymbolNotFound { symbol: symbol.to_string() });
        }
        Ok(source.replacen(symbol, payload.trim(), 1))
    }
}

pub struct MakePublic;
impl MutationStrategy for MakePublic {
    fn name(&self) -> &'static str {
        "MakePublic"
    }

    fn validate_payload(&self, _p: &str) -> MutationResult<()> {
        Ok(())
    }

    fn apply(&self, source: &str, symbol: &str, _p: &str) -> MutationResult<String> {
        let file = parse_file(source)?;

        // Top-level free function.
        for item in &file.items {
            if let Item::Fn(ItemFn { vis, sig, .. }) = item {
                if sig.ident == symbol {
                    if matches!(vis, Visibility::Public(_)) {
                        return Ok(source.to_string());
                    }

                    let vis_span = vis.span();
                    let start = line_column_to_byte_offset(source, vis_span.start());
                    let end = line_column_to_byte_offset(source, vis_span.end());

                    let mut new_source = source.to_string();
                    if start == end {
                        new_source.insert_str(start, "pub ");
                    } else {
                        new_source.replace_range(start..end, "pub");
                    }

                    return Ok(new_source);
                }
            }
        }

        // Methods/functions inside impl blocks.
        for item in &file.items {
            if let Item::Impl(item_impl) = item {
                for impl_item in &item_impl.items {
                    if let syn::ImplItem::Fn(method) = impl_item {
                        if method.sig.ident == symbol {
                            if matches!(method.vis, Visibility::Public(_)) {
                                return Ok(source.to_string());
                            }

                            let vis_span = method.vis.span();
                            let start = line_column_to_byte_offset(source, vis_span.start());
                            let end = line_column_to_byte_offset(source, vis_span.end());

                            let mut new_source = source.to_string();
                            if start == end {
                                new_source.insert_str(start, "pub ");
                            } else {
                                new_source.replace_range(start..end, "pub");
                            }

                            return Ok(new_source);
                        }
                    }
                }
            }
        }

        Err(MutationError::SymbolNotFound { symbol: symbol.to_string() })
    }
}

pub struct InsertEarlyReturn;
impl MutationStrategy for InsertEarlyReturn {
    fn name(&self) -> &'static str {
        "InsertEarlyReturn"
    }
    fn validate_payload(&self, payload: &str) -> MutationResult<()> {
        if payload.trim().is_empty() {
            return Err(MutationError::InvalidPayload(
                "Early return payload cannot be empty".into(),
            ));
        }
        Ok(())
    }
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;
        let file = parse_file(source)?;
        let target_fn = find_function_block_span(&file, symbol)?;

        let start = line_column_to_byte_offset(source, target_fn.start());
        let end = line_column_to_byte_offset(source, target_fn.end());
        let block_str = &source[start..end];
        let inner_start_rel = block_str.find('{').map_or(0, |p| p + 1);
        let abs_inner_start = start + inner_start_rel;

        let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
        let indent =
            source[line_start..start].chars().take_while(|c| c.is_whitespace()).collect::<String>();
        let inner_indent = format!("{}    ", indent);

        let formatted_return = format!("\n{}{}\n", inner_indent, payload.trim());
        let mut new_source = source.to_string();
        new_source.insert_str(abs_inner_start, &formatted_return);
        Ok(new_source)
    }
}

pub struct InsertComment;
impl MutationStrategy for InsertComment {
    fn name(&self) -> &'static str {
        "InsertComment"
    }

    fn validate_payload(&self, payload: &str) -> MutationResult<()> {
        if payload.trim().is_empty() {
            return Err(MutationError::InvalidPayload("Comment payload cannot be empty".into()));
        }
        if payload.contains('\n') {
            return Err(MutationError::InvalidPayload(
                "Comment payload must be a single line".into(),
            ));
        }
        Ok(())
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;
        let file = parse_file(source)?;
        let target_fn = find_function_block_span(&file, symbol)?;

        let start = line_column_to_byte_offset(source, target_fn.start());
        let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
        let indent =
            source[line_start..start].chars().take_while(|c| c.is_whitespace()).collect::<String>();

        let comment_line = format!("{}// {}\n", indent, payload.trim());
        let mut new_source = source.to_string();
        new_source.insert_str(line_start, &comment_line);
        Ok(new_source)
    }
}

pub struct TreeSitterParse;
impl MutationStrategy for TreeSitterParse {
    fn name(&self) -> &'static str {
        "TreeSitterParse"
    }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> {
        Ok(())
    }
    fn apply(&self, source: &str, _s: &str, _p: &str) -> MutationResult<String> {
        Ok(source.to_string())
    }
}

pub fn all_strategies() -> Vec<Box<dyn MutationStrategy>> {
    vec![
        Box::new(ReplaceFunctionBody),
        Box::new(RenameSymbol),
        Box::new(MakePublic),
        Box::new(InsertEarlyReturn),
        Box::new(InsertComment),
    ]
}

pub fn get_strategy(name: &str) -> Option<Box<dyn MutationStrategy>> {
    all_strategies().into_iter().find(|s| s.name().eq_ignore_ascii_case(name))
}

pub struct ApplySuggestedReplacement;
impl MutationStrategy for ApplySuggestedReplacement {
    fn name(&self) -> &'static str {
        "ApplySuggestedReplacement"
    }
    fn validate_payload(&self, _p: &str) -> MutationResult<()> {
        Ok(())
    }
    fn apply(&self, source: &str, _symbol: &str, _payload: &str) -> MutationResult<String> {
        Ok(source.to_string())
    }
}

/// Used by the security auditor to auto-heal vulnerabilities in Python/JS/TS files.
pub struct PolyglotSplicer;

impl PolyglotSplicer {
    pub fn splice(
        source: &str,
        start_line: usize,
        end_line: usize,
        patch: &str,
    ) -> MutationResult<String> {
        let mut lines: Vec<&str> = source.lines().collect();

        if start_line == 0 || start_line > lines.len() || end_line < start_line {
            return Err(MutationError::InvalidPayload(format!(
                "Invalid line bounds: {}-{}",
                start_line, end_line
            )));
        }

        let start_idx = start_line - 1;
        let end_idx = end_line;

        lines.drain(start_idx..end_idx);

        for (i, patch_line) in patch.lines().enumerate() {
            lines.insert(start_idx + i, patch_line);
        }

        let mut result = lines.join("\n");
        if source.ends_with('\n') && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }
}

// =========================================================================
// POLYGLOT AST MUTATION (Tree-Sitter)
// =========================================================================

use tree_sitter::Language;

pub enum PolyglotLanguage {
    Python,
    JavaScript,
    TypeScript,
}

impl PolyglotLanguage {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "py" => Some(PolyglotLanguage::Python),
            "js" => Some(PolyglotLanguage::JavaScript),
            "ts" => Some(PolyglotLanguage::TypeScript),
            _ => None,
        }
    }

    pub fn get_language(&self) -> Language {
        match self {
            PolyglotLanguage::Python => tree_sitter_python::LANGUAGE.into(),
            PolyglotLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            PolyglotLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }
}

pub struct ReplacePolyglotFunction {
    pub language: PolyglotLanguage,
}

impl MutationStrategy for ReplacePolyglotFunction {
    fn name(&self) -> &'static str {
        "ReplacePolyglotFunction"
    }

    fn validate_payload(&self, payload: &str) -> MutationResult<()> {
        if payload.trim().is_empty() {
            return Err(MutationError::InvalidPayload("Payload cannot be empty".into()));
        }
        Ok(())
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;
        let file = parse_file(source)?;
        let target_fn = find_function_block_span(&file, symbol)?;
        replace_block_body(source, target_fn, payload)
    }
}
