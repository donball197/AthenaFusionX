// apps/nexus_ast_tool/src/mutation.rs

//! Advanced AST Mutation Strategy system with robust error handling.
//! Uses syn for safe semantic boundary detection, but preserves comments
//! by slicing the original source string rather than unparsing the AST.

use crate::polyglot::SupportedLanguage;
use crate::polyglot_diff::PolyglotDiffEngine;
use proc_macro2::LineColumn;
use syn::spanned::Spanned;
use syn::{parse_file, Item, ItemFn, Visibility};
use thiserror::Error;
use tree_sitter::{Parser, Query, QueryCursor};

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

/// Reject payloads that pull in crates not already present in the original source.
/// Cheap textual pre-filter; cargo check remains the authoritative gate.
pub fn payload_introduces_unknown_crate(original: &str, payload: &str) -> Option<String> {
    use std::collections::HashSet;

    let known: HashSet<&str> = original
        .lines()
        .filter_map(|line| {
            let t = line.trim();
            if t.starts_with("use ") || t.starts_with("pub use ") {
                t.trim_start_matches("pub ")
                    .trim_start_matches("use ")
                    .split("::")
                    .next()
                    .map(|s| s.trim())
            } else {
                None
            }
        })
        .filter(|s| {
            !s.is_empty()
                && *s != "std"
                && *s != "core"
                && *s != "alloc"
                && *s != "self"
                && *s != "crate"
                && *s != "super"
        })
        .collect();

    for line in payload.lines() {
        let t = line.trim();
        if !(t.starts_with("use ") || t.starts_with("pub use ")) {
            continue;
        }
        if let Some(crate_name) = t
            .trim_start_matches("pub ")
            .trim_start_matches("use ")
            .split("::")
            .next()
            .map(|s| s.trim())
        {
            if matches!(crate_name, "std" | "core" | "alloc" | "self" | "crate" | "super") {
                continue;
            }
            if !known.contains(crate_name) {
                return Some(format!(
                    "Payload introduces unknown crate `{}` that is not present in the original source",
                    crate_name
                ));
            }
        }
    }
    None
}

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

fn find_rust_function_body(source: &str, symbol: &str) -> MutationResult<(usize, usize)> {
    let language = SupportedLanguage::Rust.tree_sitter_language();

    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| {
        MutationError::InvalidPayload(format!("Tree-sitter Rust parser unavailable: {e}"))
    })?;

    let tree = parser.parse(source, None).ok_or_else(|| {
        MutationError::InvalidPayload("Tree-sitter failed to produce a Rust parse tree".into())
    })?;

    if tree.root_node().has_error() {
        return Err(MutationError::InvalidPayload(
            "Source does not parse cleanly with Tree-sitter Rust".into(),
        ));
    }

    let query = Query::new(&language, "(function_item name: (identifier) @name)").map_err(|e| {
        MutationError::InvalidPayload(format!("Tree-sitter Rust function query failed: {e}"))
    })?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let name_capture =
        query.capture_names().iter().position(|name| *name == "name").ok_or_else(|| {
            MutationError::InvalidPayload(
                "Tree-sitter Rust function query has no name capture".into(),
            )
        })?;

    let mut matches_found = 0usize;
    let mut body_range = None;

    for query_match in &mut matches {
        for capture in query_match.captures {
            if capture.index as usize != name_capture {
                continue;
            }

            let Ok(name) = capture.node.utf8_text(source.as_bytes()) else {
                continue;
            };

            if name != symbol {
                continue;
            }

            let function_node = capture.node.parent().ok_or_else(|| {
                MutationError::InvalidPayload(format!(
                    "Tree-sitter could not resolve function node for `{symbol}`"
                ))
            })?;

            let body = function_node.child_by_field_name("body").ok_or_else(|| {
                MutationError::InvalidPayload(format!(
                    "Rust function `{symbol}` has no Tree-sitter body node"
                ))
            })?;

            matches_found += 1;
            body_range = Some(body.byte_range());
        }
    }

    match matches_found {
        0 => Err(MutationError::SymbolNotFound { symbol: symbol.to_string() }),
        1 => {
            let range = body_range.expect("body range must exist for one match");
            Ok((range.start, range.end))
        }
        _ => Err(MutationError::InvalidPayload(format!(
            "Rust symbol `{symbol}` is ambiguous: \
             Tree-sitter found {matches_found} functions with that name"
        ))),
    }
}


/// If `payload` is a complete Python function (`def name(...):` + body),
/// return only the body lines (dedented). Otherwise return the payload as-is.
/// Mirrors extract_tree_sitter_payload_body for Rust so a full `def` is not
/// re-nested inside the target function.
fn extract_python_payload_body(payload: &str) -> String {
    let trimmed = payload.trim();
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.is_empty() {
        return payload.to_string();
    }

    let first = lines[0].trim_start();
    let is_full_def = first.starts_with("def ") || first.starts_with("async def ");
    if !is_full_def {
        return payload.to_string();
    }

    let body_lines: Vec<&str> = lines.iter().skip(1).copied().collect();
    if body_lines.iter().all(|l| l.trim().is_empty()) {
        return "pass".to_string();
    }

    let min_indent = body_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0);

    body_lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                l.chars().skip(min_indent).collect::<String>()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_tree_sitter_payload_body(payload: &str) -> MutationResult<String> {
    if payload.trim().is_empty() {
        return Err(MutationError::InvalidPayload("Payload cannot be empty".into()));
    }

    let language = SupportedLanguage::Rust.tree_sitter_language();

    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| {
        MutationError::InvalidPayload(format!("Tree-sitter Rust parser unavailable: {e}"))
    })?;

    // A complete function payload is accepted directly.
    let trimmed = payload.trim();
    let source_for_parse = if trimmed.contains("fn ") {
        trimmed.to_string()
    } else {
        // Statement/body payload: parse it as the body of a synthetic
        // function. This makes Tree-sitter perform the syntax validation.
        format!("fn __nexus_mutation_target() {{\n{}\n}}", trimmed)
    };

    let tree = parser.parse(&source_for_parse, None).ok_or_else(|| {
        MutationError::InvalidPayload("Tree-sitter failed to parse mutation payload".into())
    })?;

    if tree.root_node().has_error() {
        return Err(MutationError::InvalidPayload(
            "Mutation payload does not parse cleanly with Tree-sitter Rust".into(),
        ));
    }

    let query = Query::new(&language, "(function_item body: (block) @body)").map_err(|e| {
        MutationError::InvalidPayload(format!("Tree-sitter Rust payload query failed: {e}"))
    })?;

    let mut cursor = QueryCursor::new();

    let body_node = cursor
        .matches(&query, tree.root_node(), source_for_parse.as_bytes())
        .flat_map(|m| m.captures)
        .find_map(|capture| {
            let capture_name = query.capture_names()[capture.index as usize];

            if capture_name == "body" {
                Some(capture.node)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            MutationError::InvalidPayload(
                "Tree-sitter could not locate a Rust function body in payload".into(),
            )
        })?;

    let start = body_node.start_byte();
    let end = body_node.end_byte();

    if end <= start + 1 {
        return Err(MutationError::InvalidPayload(
            "Mutation payload contains an empty Rust function body".into(),
        ));
    }

    // Tree-sitter has already identified the block boundaries, so there
    // is no string search for the first/last brace.
    let inner_start = start + 1;
    let inner_end = end - 1;

    Ok(source_for_parse[inner_start..inner_end].trim().to_string())
}

fn replace_rust_function_body(
    source: &str,
    body_range: (usize, usize),
    payload_body: &str,
) -> MutationResult<String> {
    let (start, end) = body_range;

    if start >= end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err(MutationError::InvalidPayload(
            "Tree-sitter returned an invalid Rust body byte range".into(),
        ));
    }

    let replacement = if payload_body.is_empty() {
        "{\n}".to_string()
    } else {
        let indented_body = payload_body
            .lines()
            .map(|line| if line.is_empty() { String::new() } else { format!("    {line}") })
            .collect::<Vec<_>>()
            .join("\n");

        format!("{{\n{}\n}}", indented_body)
    };

    let mut mutated = String::with_capacity(source.len() + replacement.len());

    mutated.push_str(&source[..start]);
    mutated.push_str(&replacement);
    mutated.push_str(&source[end..]);

    // The final mutation must itself be valid Rust according to the same
    // Tree-sitter parser used to locate the target.
    let language = SupportedLanguage::Rust;

    if !language.parses_cleanly(&mutated) {
        return Err(MutationError::InvalidPayload(
            "Tree-sitter rejected the mutated Rust source".into(),
        ));
    }

    Ok(mutated)
}

impl ReplaceFunctionBody {
    /// Validate a model body payload against the source language selected
    /// from the target path. The mutation engine, not the model, owns the
    /// language-specific boundary contract.
    pub fn validate_for_path(
        file_path: &std::path::Path,
        source: &str,
        symbol: &str,
        payload: &str,
    ) -> MutationResult<()> {
        if payload.trim().is_empty() {
            return Err(MutationError::InvalidPayload("Payload cannot be empty".into()));
        }

        let language = SupportedLanguage::from_path(file_path).ok_or_else(|| {
            MutationError::InvalidPayload(format!(
                "Unsupported source language: {}",
                file_path.display()
            ))
        })?;

        let context = PolyglotDiffEngine::analyze_source_with_language(language, source, symbol)
            .map_err(MutationError::Other)?;

        let normalized = normalize_polyglot_payload(language, payload, &context.original_body);

        PolyglotDiffEngine::apply(&context, source, &normalized)
            .map(|_| ())
            .map_err(MutationError::Other)
    }

    /// Apply a body replacement through the language-neutral Tree-sitter
    /// diff engine. No alternate boundary detector is permitted here.
    pub fn apply_for_path(
        file_path: &std::path::Path,
        source: &str,
        symbol: &str,
        payload: &str,
    ) -> MutationResult<String> {
        if payload.trim().is_empty() {
            return Err(MutationError::InvalidPayload("Payload cannot be empty".into()));
        }

        let language = SupportedLanguage::from_path(file_path).ok_or_else(|| {
            MutationError::InvalidPayload(format!(
                "Unsupported source language: {}",
                file_path.display()
            ))
        })?;

        let context = PolyglotDiffEngine::analyze_source_with_language(language, source, symbol)
            .map_err(MutationError::Other)?;

        // The payload may be either a bare body block (e.g. `a + b`) or a
        // complete function definition (e.g. `pub fn add(...) -> i32 { a + b }`).
        // Normalize both to the inner body content before applying
        // indentation, so a full function definition is not re-nested inside
        // the target function. This mirrors the legacy `apply` path, which
        // already calls `extract_tree_sitter_payload_body`. Non-Rust
        // languages pass through unchanged.
        let body_payload = match language {
            SupportedLanguage::Rust => extract_tree_sitter_payload_body(payload)?,
            SupportedLanguage::Python => extract_python_payload_body(payload),
            _ => payload.to_string(),
        };

        let normalized =
            normalize_polyglot_payload(language, &body_payload, &context.original_body);

        PolyglotDiffEngine::apply(&context, source, &normalized)
            .map(|result| result.mutated_source)
            .map_err(MutationError::Other)
    }
}

fn normalize_polyglot_payload(
    language: SupportedLanguage,
    payload: &str,
    original_body: &str,
) -> String {
    let trimmed = payload.trim();

    match language {
                SupportedLanguage::Python => {
            // Python bodies require indentation but no enclosing braces.
            // Preserve relative indentation inside the payload; only shift
            // the whole block to the target body's base indent. Flattening
            // every line to the same indent (the old behavior) turns
            //   if b == 0:
            //       return 0
            // into an IndentationError.
            let base_indent = original_body
                .lines()
                .find(|line| {
                    let t = line.trim();
                    !t.is_empty() && t != "{"
                })
                .map(|line| line.chars().take_while(|c| c.is_whitespace()).collect::<String>())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "    ".to_string());

            let min_payload_indent = trimmed
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
                .min()
                .unwrap_or(0);

            trimmed
                .lines()
                .map(|line| {
                    if line.trim().is_empty() {
                        String::new()
                    } else {
                        let line_indent = line.chars().take_while(|c| c.is_whitespace()).count();
                        let relative = line_indent.saturating_sub(min_payload_indent);
                        format!(
                            "{base_indent}{}{}",
                            " ".repeat(relative),
                            line.trim()
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => {
            // Brace-delimited languages need the replacement body aligned with
            // the target function's existing body indentation.
            let indent = original_body
                .lines()
                .find(|line| {
                    let t = line.trim();
                    // Skip the opening `{` of the target body. It is the first
                    // non-empty line of `original_body` for brace-delimited
                    // languages, but it has zero leading whitespace, so deriving
                    // indent from it produces an empty prefix that silently
                    // shadows the fallback below.
                    !t.is_empty() && t != "{"
                })
                .map(|line| line.chars().take_while(|c| c.is_whitespace()).collect::<String>())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "    ".to_string());

            let body = if trimmed.starts_with('{') && trimmed.ends_with('}') {
                trimmed[1..trimmed.len() - 1].trim().to_string()
            } else {
                trimmed.to_string()
            };

            let indented_body = body
                .lines()
                .map(|line| {
                    if line.trim().is_empty() {
                        String::new()
                    } else {
                        format!("{indent}{}", line.trim())
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            // The closing brace belongs at the enclosing scope's indent, not
            // the body's indent -- these are different levels whenever the
            // target is nested (e.g. an impl method, where the body is
            // 8-space indented but the closing brace sits at 4 spaces to
            // match `fn`). Derive it from original_body's own closing-brace
            // line rather than reusing the body-statement indent, which
            // previously left every closing brace over-indented by one level.
            let closing_indent = original_body
                .lines()
                .last()
                .map(|line| line.chars().take_while(|c| c.is_whitespace()).collect::<String>())
                .unwrap_or_default();

            if indented_body.is_empty() {
                format!("{{\n{closing_indent}}}")
            } else {
                format!("{{\n{indented_body}\n{closing_indent}}}")
            }
        }
    }
}

impl MutationStrategy for ReplaceFunctionBody {
    fn name(&self) -> &'static str {
        "ReplaceFunctionBody"
    }

    fn validate_payload(&self, payload: &str) -> MutationResult<()> {
        extract_tree_sitter_payload_body(payload).map(|_| ())
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        let payload_body = extract_tree_sitter_payload_body(payload)?;
        let body_range = find_rust_function_body(source, symbol)?;

        replace_rust_function_body(source, body_range, &payload_body)
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

#[cfg(test)]
mod mutation_tests {
    use super::*;

    #[test]
    fn replace_function_body_uses_tree_sitter_byte_range() {
        let source = r#"
fn target(value: i32) -> i32 {
    let nested = {
        let text = "{ not a Rust block }";
        value + 1
    };
    nested
}

fn unrelated() -> &'static str {
    "UNCHANGED"
}
"#;

        let strategy = ReplaceFunctionBody;

        let mutated = strategy
            .apply(
                source,
                "target",
                r#"
let nested = {
    let text = "{ still not a block }";
    value + 41
};
nested
"#,
            )
            .expect("Tree-sitter mutation should succeed");

        assert!(mutated.contains("value + 41"));
        assert!(!mutated.contains("value + 1"));
        assert!(mutated.contains(
            r#"fn unrelated() -> &'static str {
    "UNCHANGED"
}"#
        ));

        assert!(
            SupportedLanguage::Rust.parses_cleanly(&mutated),
            "mutated source must remain valid Rust"
        );
    }

    #[test]
    fn replace_impl_method_body() {
        let source = r#"
struct Calculator;

impl Calculator {
    fn calculate(&self, value: i32) -> i32 {
        value + 10
    }

    fn unrelated(&self) -> &'static str {
        "UNCHANGED"
    }
}
"#;

        let strategy = ReplaceFunctionBody;

        let mutated = strategy
            .apply(source, "calculate", "value + 99")
            .expect("Tree-sitter should locate impl method");

        assert!(mutated.contains("value + 99"));
        assert!(!mutated.contains("value + 10"));
        assert!(mutated.contains(
            r#"fn unrelated(&self) -> &'static str {
        "UNCHANGED"
    }"#
        ));

        assert!(SupportedLanguage::Rust.parses_cleanly(&mutated));
    }

    #[test]
    fn unicode_before_target_does_not_corrupt_byte_range() {
        let source = r#"
const DESCRIPTION: &str = "日本語 — AthenaFusionX 🚀";

fn target() -> &'static str {
    "OLD"
}

fn unrelated() -> &'static str {
    "UNCHANGED"
}
"#;

        let strategy = ReplaceFunctionBody;

        let mutated = strategy
            .apply(source, "target", r#""NEW""#)
            .expect("Unicode before target must not break byte offsets");

        assert!(mutated.contains(
            r#"fn target() -> &'static str {
    "NEW"
}"#
        ));
        assert!(mutated.contains(
            r#"fn unrelated() -> &'static str {
    "UNCHANGED"
}"#
        ));
        assert!(mutated.contains("日本語 — AthenaFusionX 🚀"));

        assert!(SupportedLanguage::Rust.parses_cleanly(&mutated));
    }

    #[test]
    fn complete_function_payload_is_supported() {
        let source = r#"
fn target(value: i32) -> i32 {
    value + 1
}
"#;

        let strategy = ReplaceFunctionBody;

        let mutated = strategy
            .apply(
                source,
                "target",
                r#"
fn target(value: i32) -> i32 {
    value * 2
}
"#,
            )
            .expect("complete function payload should be accepted");

        assert!(mutated.contains("value * 2"));
        assert!(!mutated.contains("value + 1"));
        assert!(SupportedLanguage::Rust.parses_cleanly(&mutated));
    }

    #[test]
    fn apply_and_apply_for_path_agree_on_flush_left_payload() {
        use std::path::Path;

        let source = r#"
fn target(value: i32) -> i32 {
    value + 1
}
"#;

        let path = Path::new("dummy.rs");
        let payload = "value * 2";

        let via_apply =
            ReplaceFunctionBody.apply(source, "target", payload).expect("apply must succeed");

        let via_for_path = ReplaceFunctionBody::apply_for_path(path, source, "target", payload)
            .expect("apply_for_path must succeed");

        assert_eq!(
            via_apply, via_for_path,
            "flush-left payload must produce identical output on both paths"
        );
    }

    #[test]
    fn apply_for_path_normalizes_pre_indented_payload() {
        use std::path::Path;

        let source = r#"
fn target(value: i32) -> i32 {
    value + 1
}
"#;

        let path = Path::new("dummy.rs");

        // Mimics the body-shaped payloads emitted by the pipeline LLM.
        let payload = "// doubled\n    value * 2";

        let mutated = ReplaceFunctionBody::apply_for_path(path, source, "target", payload)
            .expect("apply_for_path must succeed");

        let expected = r#"
fn target(value: i32) -> i32 {
    // doubled
    value * 2
}
"#;

        assert_eq!(mutated, expected, "apply_for_path must normalize model-provided indentation");
    }

    #[test]
    fn apply_for_path_extracts_full_function_payload() {
        use std::path::Path;

        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let path = Path::new("dummy.rs");

        // A complete function definition, as produced by the pipeline LLM.
        // apply_for_path must extract its body and re-indent to match the
        // target, NOT re-nest the function inside itself.
        let payload = "pub fn add(a: i32, b: i32) -> i32 { a + b }";

        let mutated = ReplaceFunctionBody::apply_for_path(path, source, "add", payload)
            .expect("apply_for_path must accept a full function payload");

        assert_eq!(mutated, source, "applying the canonical body must be a byte-level no-op");
        assert!(
            !mutated.contains("pub fn add(a: i32, b: i32) -> i32 {\n    pub fn add"),
            "apply_for_path must not nest the function inside itself"
        );
    }

    #[test]
    fn apply_for_path_extracts_documented_function_payload() {
        use std::path::Path;

        let source = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let path = Path::new("dummy.rs");

        let payload =
            "pub fn add(a: i32, b: i32) -> i32 {\n    // Doubles as documentation.\n    a + b\n}";

        let mutated = ReplaceFunctionBody::apply_for_path(path, source, "add", payload)
            .expect("apply_for_path must accept a documented function payload");

        let expected =
            "pub fn add(a: i32, b: i32) -> i32 {\n    // Doubles as documentation.\n    a + b\n}\n";

        assert_eq!(
            mutated, expected,
            "documented payload must produce exactly one function-body indent level"
        );
    }

    #[test]
    fn apply_for_path_reapplying_same_body_is_idempotent() {
        use std::path::Path;

        let source = r#"
fn target(value: i32) -> i32 {
    value + 1
}
"#;

        let payload = "// doubled\n    value * 2";
        let path = Path::new("dummy.rs");

        let first = ReplaceFunctionBody::apply_for_path(path, source, "target", payload)
            .expect("first application must succeed");

        assert_ne!(first, source, "first application must mutate the source");

        let second = ReplaceFunctionBody::apply_for_path(path, &first, "target", payload)
            .expect("second application must succeed");

        assert_eq!(first, second, "reapplying the same function body must be idempotent");

        assert!(
            SupportedLanguage::Rust.parses_cleanly(&second),
            "idempotent result must remain valid Rust"
        );
    }

    #[test]
    fn apply_for_path_handles_impl_method_indent() {
        use std::path::Path;

        let source = r#"
struct Calculator;

impl Calculator {
    fn calculate(&self, value: i32) -> i32 {
        value + 10
    }
}
"#;

        let path = Path::new("dummy.rs");
        let payload = "value * 2";

        let mutated = ReplaceFunctionBody::apply_for_path(path, source, "calculate", payload)
            .expect("apply_for_path must succeed for impl methods");

        let expected = r#"
struct Calculator;

impl Calculator {
    fn calculate(&self, value: i32) -> i32 {
        value * 2
    }
}
"#;

        assert_eq!(
            mutated, expected,
            "apply_for_path must preserve the impl method's 8-space body indent"
        );
    }

    #[test]
    fn invalid_payload_is_rejected_by_tree_sitter() {
        let strategy = ReplaceFunctionBody;

        let result = strategy.validate_payload("let value = ;");

        assert!(result.is_err());
    }

    #[test]
    fn missing_symbol_is_rejected() {
        let source = r#"
fn existing() {
    let value = 1;
}
"#;

        let strategy = ReplaceFunctionBody;

        let result = strategy.apply(source, "missing", "let value = 2;");

        assert!(matches!(
            result,
            Err(MutationError::SymbolNotFound { symbol })
                if symbol == "missing"
        ));
    }

    #[test]
    fn duplicate_function_names_are_rejected_as_ambiguous() {
        let source = r#"
fn duplicate() {
    let value = 1;
}

mod nested {
    fn duplicate() {
        let value = 2;
    }
}
"#;

        let strategy = ReplaceFunctionBody;

        let result = strategy.apply(source, "duplicate", "let value = 3;");

        assert!(matches!(
            result,
            Err(MutationError::InvalidPayload(message))
                if message.contains("ambiguous")
        ));
    }

    #[test]
    fn braces_inside_strings_and_comments_are_safe() {
        let source = r#"
fn target() {
    let first = "{";
    let second = "}";
    // { this is not the function boundary }
    if true {
        println!("nested {{ braces }}");
    }
}

fn unrelated() {
    println!("UNCHANGED");
}
"#;

        let strategy = ReplaceFunctionBody;

        let mutated = strategy
            .apply(
                source,
                "target",
                r#"
let message = "{ replacement }";
if true {
    println!("still nested");
}
"#,
            )
            .expect("Tree-sitter should identify the actual body");

        assert!(mutated.contains(r#"let message = "{ replacement }";"#));
        assert!(mutated.contains(r#"println!("still nested");"#));
        assert!(!mutated.contains(r#"let first = "{";"#));
        assert!(mutated.contains(r#"println!("UNCHANGED");"#));

        assert!(SupportedLanguage::Rust.parses_cleanly(&mutated));
    }
}

#[cfg(test)]
mod python_patch_regression {
    use super::*;
    use std::path::Path;

    #[test]
    fn python_full_def_payload_preserves_nested_indent() {
        let source = "def divide(a, b):\n    return a / b\n";
        let path = Path::new("math_ops.py");
        let payload = "def divide(a, b):\n    if b == 0:\n        return 0\n    return a / b\n";

        let mutated = ReplaceFunctionBody::apply_for_path(path, source, "divide", payload)
            .expect("Python apply_for_path must succeed");

        eprintln!("=== MUTATED ===\n{mutated}=== END ===");

        let expected = "def divide(a, b):\n    if b == 0:\n        return 0\n    return a / b\n";
        assert_eq!(
            mutated, expected,
            "nested indent under `if` must be preserved; got:\n{mutated}"
        );
    }
}
