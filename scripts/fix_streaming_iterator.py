import os

ast_path = "src/ast_editor.rs"
if os.path.exists(ast_path):
    with open(ast_path, "r") as f:
        code = f.read()
    
    # Add StreamingIterator to the imports
    old_import = "use tree_sitter::{Parser, Query, QueryCursor};"
    new_import = "use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};"
    
    if old_import in code:
        code = code.replace(old_import, new_import)
        with open(ast_path, "w") as f:
            f.write(code)
        print("\x1b[32m>> [SUTURE] StreamingIterator trait imported. DNA purified.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] Import already updated or not found.\x1b[0m")
