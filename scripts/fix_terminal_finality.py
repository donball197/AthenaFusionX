import os

# --- 1. Repair src/fs_watcher.rs ---
watcher_path = "src/fs_watcher.rs"
if os.path.exists(watcher_path):
    with open(watcher_path, "r") as f:
        code = f.read()
    
    # Fix imports: Add Debouncer and NoCache
    code = code.replace(
        "use notify_debouncer_full::{new_debouncer, notify::*, DebounceEventResult};",
        "use notify_debouncer_full::{new_debouncer, notify::*, DebounceEventResult, Debouncer, NoCache};"
    )
    
    # Fix signature: Use anyhow::Result to avoid notify::Result conflict
    code = code.replace(
        "pub fn ignite(tx: broadcast::Sender<String>) -> Result<Debouncer<RecommendedWatcher, NoCache>, Error>",
        "pub fn ignite(tx: broadcast::Sender<String>) -> anyhow::Result<Debouncer<RecommendedWatcher, NoCache>>"
    )
    
    # Fix watch call: Remove .watcher() as per deprecation warning
    code = code.replace(
        "debouncer.watcher().watch(std::path::Path::new(\"./src\"), RecursiveMode::Recursive)?;",
        "debouncer.watch(std::path::Path::new(\"./src\"), RecursiveMode::Recursive)?;"
    )
    
    with open(watcher_path, "w") as f:
        f.write(code)
    print("\x1b[32m>> [REPAIR] src/fs_watcher.rs re-aligned with notify-v8 API.\x1b[0m")

# --- 2. Repair src/ast_editor.rs ---
ast_path = "src/ast_editor.rs"
if os.path.exists(ast_path):
    with open(ast_path, "r") as f:
        code = f.read()
    
    # Fix loop: Iterate directly to satisfy QueryMatches iterator requirements
    old_loop = """        let matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());

        for m in matches {"""
    
    new_loop = """        for m in cursor.matches(&query, tree.root_node(), source_code.as_bytes()) {"""
    
    if old_loop in code:
        code = code.replace(old_loop, new_loop)
        with open(ast_path, "w") as f:
            f.write(code)
        print("\x1b[32m>> [REPAIR] src/ast_editor.rs re-aligned with tree-sitter-v0.24.\x1b[0m")
    else:
        print("\x1b[33m>> [SKIP] src/ast_editor.rs loop already optimized or not found.\x1b[0m")
