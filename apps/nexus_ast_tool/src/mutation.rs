//! Advanced AST Mutation Strategy system with robust error handling.
//! Uses syn for safe semantic boundary detection, but preserves comments
//! by slicing the original source string rather than unparsing the AST.

use proc_macro2::LineColumn;
use syn::spanned::Spanned;
use syn::{Item, ItemFn, Visibility, parse_file};
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

        let target_fn = file.items.iter().find_map(|item| {
            match item {
                Item::Fn(ItemFn { sig, block, .. }) if sig.ident == symbol => Some(block.span()),
                Item::Impl(syn::ItemImpl { items, .. }) => {
                    items.iter().find_map(|i| {
                        if let syn::ImplItem::Fn(method) = i {
                            if method.sig.ident == symbol {
                                return Some(method.block.span());
                            }
                        }
                        None
                    })
                }
                _ => None,
            }
        }).ok_or_else(|| MutationError::SymbolNotFound { symbol: symbol.to_string() })?;

        let start = line_column_to_byte_offset(source, target_fn.start());
        let end = line_column_to_byte_offset(source, target_fn.end());

        let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
        let indent = source[line_start..start].chars().take_while(|c| c.is_whitespace()).collect::<String>();

        let inner = format!("{}    ", indent);
        let formatted_payload = payload.lines().map(|l| {
            if l.trim().is_empty() { "".into() } else { format!("{}{}", inner, l.trim()) }
        }).collect::<Vec<_>>().join("
");

        let new_block_str = format!("{{
{}
{}}}", formatted_payload, indent);

        let mut new_source = source.to_string();
        new_source.replace_range(start..end, &new_block_str);
        Ok(new_source)
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
        // For simple identifier swaps, we can safely mutate the AST and unparse,
        // but to preserve comments perfectly, we'll replace the specific ident span.
        let file = parse_file(source)?;
        let mut found = false;
        let mut new_source = source.to_string();

        // Note: This simple version only renames the function declaration, not call sites.
        // For a full rename, a syntax tree visitor is needed.
        for item in &file.items {
            if let Item::Fn(ItemFn { sig, .. }) = item {
                if sig.ident == symbol {
                    let span = sig.ident.span();
                    let start =
                        line_column_to_byte_offset(source, span.start());
                    let end = line_column_to_byte_offset(source, span.end());
                    new_source.replace_range(start..end, payload);
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return Err(MutationError::SymbolNotFound { symbol: symbol.to_string() });
        }
        Ok(new_source)
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
    fn validate_payload(&self, _p: &str) -> MutationResult<()> {
        Ok(())
    }
    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        let file = parse_file(source)?;
        let target_fn = file.items.iter().find_map(|item| {
            match item {
                Item::Fn(ItemFn { sig, block, .. }) if sig.ident == symbol => {
                    Some(block.brace_token.span.open().start())
                }
                Item::Impl(syn::ItemImpl { items, .. }) => {
                    items.iter().find_map(|i| {
                        if let syn::ImplItem::Fn(method) = i {
                            if method.sig.ident == symbol {
                                return Some(method.block.brace_token.span.open().start());
                            }
                        }
                        None
                    })
                }
                _ => None,
            }
        }).ok_or_else(|| MutationError::SymbolNotFound { symbol: symbol.to_string() })?;

        let brace_offset = line_column_to_byte_offset(source, target_fn);

        // Dynamically extract indentation based on the brace offset
        let line_start = source[..brace_offset].rfind('\n').map_or(0, |pos| pos + 1);
        let indent = source[line_start..brace_offset].chars().take_while(|c| c.is_whitespace()).collect::<String>();
        
        let mut new_source = source.to_string();
        // Use extracted indent + 4 spaces for the return statement
        new_source.insert_str(brace_offset + 1, &format!("
{}    return {};", indent, payload));

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
        if payload.contains("
") {
            return Err(MutationError::InvalidPayload("Comment payload must be a single line".into()));
        }
        Ok(())
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;
        let file = parse_file(source)?;

        let target_span_start = file.items.iter().find_map(|item| {
            match item {
                Item::Fn(ItemFn { sig, .. }) if sig.ident == symbol => {
                    Some(sig.ident.span().start())
                }
                Item::Impl(syn::ItemImpl { items, .. }) => {
                    items.iter().find_map(|i| {
                        if let syn::ImplItem::Fn(method) = i {
                            if method.sig.ident == symbol {
                                return Some(method.sig.ident.span().start());
                            }
                        }
                        None
                    })
                }
                _ => None,
            }
        }).ok_or_else(|| MutationError::SymbolNotFound { symbol: symbol.to_string() })?;

        let start = line_column_to_byte_offset(source, target_span_start);
        let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);

        let indent: String = source[line_start..start].chars().take_while(|c| c.is_whitespace()).collect();
        let comment = format!("{}// {}", indent, payload.trim());

        let mut out = String::with_capacity(source.len() + comment.len() + 1);
        out.push_str(&source[..line_start]);
        out.push_str(&comment);
        out.push('\n');
        out.push_str(&source[line_start..]);

        Ok(out)
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
        // Payload is a JSON string containing { "byte_start": usize, "byte_end": usize, "suggestion": String }
        // We will handle the byte offsets directly in the cortical_loop to avoid messy JSON serialization here.
        // For now, just return the source unmodified as a fallback.
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

        // Validate bounds (1-indexed to 0-indexed)
        if start_line == 0 || start_line > lines.len() || end_line < start_line {
            return Err(MutationError::InvalidPayload(format!(
                "Invalid line bounds: {}-{}",
                start_line, end_line
            )));
        }

        let start_idx = start_line - 1;
        let end_idx = end_line; // end_line is inclusive, so we remove up to end_line (which is end_idx in 0-indexed exclusive)

        // Remove the vulnerable lines
        lines.drain(start_idx..end_idx);

        // Insert the patch lines in their place
        for (i, patch_line) in patch.lines().enumerate() {
            lines.insert(start_idx + i, patch_line);
        }

        // Rejoin the lines
        let mut result = lines.join("\n");
        // Ensure file ends with a newline if the original did
        if source.ends_with('\n') && !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    }
}

// =========================================================================
// POLYGLOT AST MUTATION (Tree-Sitter)
// =========================================================================

use tree_sitter::{Language, Node, Parser};

/// Enum to map file extensions to Tree-sitter languages safely
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
        // For a strict check, we could try to parse the payload as a function_definition here.
        // For now, we trust the LLM's output and let the AST rescan verify it.
        Ok(())
    }

    fn apply(&self, source: &str, symbol: &str, payload: &str) -> MutationResult<String> {
        self.validate_payload(payload)?;

        let mut parser = Parser::new();
        let language = self.language.get_language();
        parser
            .set_language(&language)
            .map_err(|e| MutationError::TreeSitterError(e.to_string()))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| MutationError::TreeSitterError("Failed to parse source code".into()))?;

        let root_node = tree.root_node();

        // Walk the AST to find the function definition matching the symbol name
        let mut cursor = root_node.walk();
        let source_bytes = source.as_bytes();

        // Recursive function to traverse the tree
        fn traverse_node<'a>(
            node: Node<'a>,
            cursor: &mut tree_sitter::TreeCursor<'a>,
            symbol: &str,
            source_bytes: &[u8],
        ) -> Option<Node<'a>> {
            // Check if current node is a function/method definition
            let kind = node.kind();
            if kind == "function_definition"
                || kind == "function_declaration"
                || kind == "method_definition"
            {
                // Try to get the 'name' child (Python/JS/TS all use 'name' field for identifiers)
                if let Some(name_node) = node.child_by_field_name("name") {
                    if name_node.kind() == "identifier" {
                        let node_text = name_node.utf8_text(source_bytes).unwrap_or("");
                        if node_text == symbol {
                            return Some(node);
                        }
                    }
                }
            }

            // Recurse into children
            if cursor.goto_first_child() {
                loop {
                    let child_node = cursor.node();
                    if let Some(found) = traverse_node(child_node, cursor, symbol, source_bytes) {
                        return Some(found);
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }

            None
        }

        let target_node = traverse_node(root_node, &mut cursor, symbol, source_bytes);

        let target_node = target_node.ok_or_else(|| MutationError::SymbolNotFound {
            symbol: format!("{} (in Polyglot AST)", symbol),
        })?;

        // 1. Get the exact byte offsets of the function node
        let start_byte = target_node.start_byte();
        let end_byte = target_node.end_byte();

        // 2. Extract the indentation prefix of the function
        let line_start = source[..start_byte].rfind('\n').map_or(0, |pos| pos + 1);
        let indent = source[line_start..start_byte]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect::<String>();

        // 3. Format the new payload with the extracted indentation
        let formatted_payload = payload
            .lines()
            .map(|l| if l.trim().is_empty() { "".into() } else { format!("{}{}", indent, l) })
            .collect::<Vec<_>>()
            .join("\n");

        // 4. Splice the new block into the original source string
        let mut new_source = source.to_string();
        new_source.replace_range(start_byte..end_byte, &formatted_payload);

        Ok(new_source)
    }
}
