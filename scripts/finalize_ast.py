import os

ast_path = "src/ast_editor.rs"
if os.path.exists(ast_path):
    with open(ast_path, "r") as f:
        code = f.read()
    
    # The Fix: Use while let Some(m) = matches.next() instead of for m in matches
    # This bypasses the IntoIterator constraint that is failing in tree-sitter v0.24
    old_block = """        let mut cursor = QueryCursor::new();
        for m in cursor.matches(&query, tree.root_node(), source_code.as_bytes()) {"""
    
    new_block = """        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());
        while let Some(m) = matches.next() {"""
    
    if old_block in code:
        code = code.replace(old_block, new_block)
    else:
        # Fallback in case of slight formatting differences
        code = code.replace(
            "for m in cursor.matches(&query, tree.root_node(), source_code.as_bytes()) {",
            "let mut matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());\n        while let Some(m) = matches.next() {"
        )
    
    with open(ast_path, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] src/ast_editor.rs iterator re-aligned. Semantic Engine ready.\x1b[0m")
