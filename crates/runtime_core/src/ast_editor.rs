use anyhow::{anyhow, Result};
use std::fs;
use tree_sitter::{Parser, Query, QueryCursor};
pub struct AstEditor;
impl AstEditor {
    /// update_function: Finds a function by name and replaces its entire body.
    /// This is the "Semantic Precision" upgrade to replace simple string diffs.
    pub fn read_function(path: &str, function_name: &str) -> Result<String> {
        let source_code = fs::read_to_string(path)?;
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        let tree = parser
            .parse(&source_code, None)
            .ok_or_else(|| anyhow!("AST_PARSE_FAIL: Could not parse {}", path))?;
        let query_str = format!(
            "(function_item name: (identifier) @name (#eq? @name \"{}\")) @func",
            function_name
        );
        let query = Query::new(&tree_sitter_rust::LANGUAGE.into(), &query_str)?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());
        for m in matches {
            for capture in m.captures {
                if capture.index == 1 {
                    let node = capture.node;
                    let start = node.start_byte();
                    let end = node.end_byte();
                    return Ok(source_code[start..end].to_string());
                }
            }
        }
        Err(anyhow!("FUNCTION_NOT_FOUND: Could not find function '{}' in {}", function_name, path))
    }

    /// Validates that `new_body` is exactly one function definition named
    /// `function_name` and nothing else -- no top-level `use` statements, no
    /// extra functions, no `mod` blocks alongside it. Without this, a
    /// Strategist proposal whose `new_body` accidentally (or confidently)
    /// contains a whole re-typed file gets spliced in verbatim: real
    /// incident -- new_body containing `use axum::{...}`, a duplicate
    /// `#[tokio::main]`, and a `mod tests` block all landed inside what was
    /// meant to be just the function body, producing duplicate-import
    /// compile errors. A naive "does it parse" check does not catch this,
    /// since each individual piece (a use statement, a second `fn main`) is
    /// syntactically valid Rust on its own -- duplicate-definition is a
    /// semantic error tree-sitter's grammar-only parser never sees. This
    /// check instead requires new_body to parse as a standalone fragment
    /// containing exactly one named top-level item, and that item must be a
    /// function_item matching function_name.
    fn validate_single_function_body(new_body: &str, function_name: &str) -> Result<()> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        let fragment_tree = parser.parse(new_body, None).ok_or_else(|| {
            anyhow!(
                "PATCH_VALIDATION_FAILED: new_body for '{}' could not be parsed as Rust",
                function_name
            )
        })?;

        let root = fragment_tree.root_node();
        let mut walker = root.walk();
        let mut named_children = root.named_children(&mut walker);
        let first = named_children.next().ok_or_else(|| {
            anyhow!(
                "PATCH_VALIDATION_FAILED: new_body for '{}' is empty or contains no recognizable Rust item",
                function_name
            )
        })?;

        if named_children.next().is_some() {
            return Err(anyhow!(
                "PATCH_VALIDATION_FAILED: new_body for '{}' contains more than one top-level item \
                 (e.g. extra use statements, functions, or a mod block splice into the middle of a \
                 single-function patch) -- new_body must contain exactly one function definition",
                function_name
            ));
        }

        if first.kind() != "function_item" {
            return Err(anyhow!(
                "PATCH_VALIDATION_FAILED: new_body for '{}' is a '{}', not a function_item -- \
                 patch_function only replaces function bodies, not other item kinds",
                function_name,
                first.kind()
            ));
        }

        let name_node = first.child_by_field_name("name").ok_or_else(|| {
            anyhow!("PATCH_VALIDATION_FAILED: could not read the name of the function in new_body")
        })?;
        let actual_name = &new_body[name_node.start_byte()..name_node.end_byte()];
        if actual_name != function_name {
            return Err(anyhow!(
                "PATCH_VALIDATION_FAILED: new_body defines function '{}' but the patch target is '{}' -- \
                 refusing to rename a function via patch_function",
                actual_name,
                function_name
            ));
        }

        Ok(())
    }

    pub fn update_function(path: &str, function_name: &str, new_body: &str) -> Result<String> {
        Self::validate_single_function_body(new_body, function_name)?;

        let source_code = fs::read_to_string(path)?;
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        let tree = parser
            .parse(&source_code, None)
            .ok_or_else(|| anyhow!("AST_PARSE_FAIL: Could not parse {}", path))?;
        // Query to find the specific function node
        let query_str = format!(
            "(function_item name: (identifier) @name (#eq? @name \"{}\")) @func",
            function_name
        );
        let query = Query::new(&tree_sitter_rust::LANGUAGE.into(), &query_str)?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());
        for m in matches {
            for capture in m.captures {
                if capture.index == 1 {
                    // The @func capture
                    let node = capture.node;
                    let start = node.start_byte();
                    let end = node.end_byte();
                    let mut new_code = source_code.clone();
                    new_code.replace_range(start..end, new_body);

                    // Safety net beyond the standalone-fragment check above:
                    // confirm the *spliced* result still parses cleanly.
                    // Catches cases the isolated fragment check can't see,
                    // e.g. new_body being syntactically fine alone but
                    // producing a parse error once merged with surrounding
                    // context (mismatched braces at the splice boundary).
                    let mut verify_parser = Parser::new();
                    verify_parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
                    if let Some(verify_tree) = verify_parser.parse(&new_code, None) {
                        if verify_tree.root_node().has_error() {
                            return Err(anyhow!(
                                "PATCH_VALIDATION_FAILED: applying new_body to '{}' would leave {} with a Rust syntax error",
                                function_name,
                                path
                            ));
                        }
                    }

                    return Ok(new_code);
                }
            }
        }
        Err(anyhow!("FUNCTION_NOT_FOUND: Could not find function '{}' in {}", function_name, path))
    }
}
