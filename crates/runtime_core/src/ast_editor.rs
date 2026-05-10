use anyhow::{anyhow, Result};
use std::fs;
use tree_sitter::{Parser, Query, QueryCursor};

pub struct AstEditor;

impl AstEditor {
    /// update_function: Finds a function by name and replaces its entire body.
    /// This is the "Semantic Precision" upgrade to replace simple string diffs.
    pub fn update_function(path: &str, function_name: &str, new_body: &str) -> Result<String> {
        let source_code = fs::read_to_string(path)?;
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::language().into())?;

        let tree = parser
            .parse(&source_code, None)
            .ok_or_else(|| anyhow!("AST_PARSE_FAIL: Could not parse {}", path))?;

        // Query to find the specific function node
        let query_str = format!(
            "(function_item name: (identifier) @name (#eq? @name \"{}\")) @func",
            function_name
        );
        let query = Query::new(&tree_sitter_rust::language().into(), &query_str)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                if capture.index == 1 {
                    // The @func capture
                    let node = capture.node;
                    let start = node.start_byte();
                    let end = node.end_byte();

                    let mut new_code = source_code.clone();
                    new_code.replace_range(start..end, new_body);
                    return Ok(new_code);
                }
            }
        }

        Err(anyhow!("FUNCTION_NOT_FOUND: Could not find function '{}' in {}", function_name, path))
    }
}
