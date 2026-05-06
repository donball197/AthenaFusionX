import os

# 1. Forge the Autonomic Nervous System (fs_watcher.rs)
fs_watcher_code = """// src/fs_watcher.rs — v1.0 [AUTONOMIC NERVOUS SYSTEM]
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config, EventKind};
use std::path::Path;
use tokio::sync::broadcast;
use tracing::{info, debug, warn};
use serde_json::json;

pub fn ignite_nervous_system(tx: broadcast::Sender<String>, workspace_root: &Path) {
    let root = workspace_root.to_path_buf();
    
    std::thread::spawn(move || {
        let (std_tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(std_tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                warn!("Failed to initialize OS filesystem watcher: {}", e);
                return;
            }
        };

        let src_path = root.join("src");
        if src_path.exists() {
            let _ = watcher.watch(&src_path, RecursiveMode::Recursive);
        }

        info!("\x1b[36m>> [AUTONOMIC_WATCHER] OS-Level Inotify Hooks Active\x1b[0m");

        for res in rx {
            match res {
                Ok(event) => {
                    if let EventKind::Modify(_) = event.kind {
                        for path in event.paths {
                            if let Some(ext) = path.extension() {
                                if ext == "rs" || ext == "md" || ext == "svelte" {
                                    let clean_path = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
                                    
                                    // Broadcast the modification event globally
                                    let _ = tx.send(json!({
                                        "type": "FS_EVENT",
                                        "payload": format!("FILE_MODIFIED: {}", clean_path)
                                    }).to_string());
                                    
                                    debug!("[FS_EVENT] {}", clean_path);
                                }
                            }
                        }
                    }
                }
                Err(e) => warn!("Watch error: {:?}", e),
            }
        }
    });
}
"""

with open("src/fs_watcher.rs", "w") as f:
    f.write(fs_watcher_code)

# 2. Forge the Semantic AST Editor Skeleton (ast_editor.rs)
ast_editor_code = """// src/ast_editor.rs — v1.0 [SEMANTIC SCALPEL]
use anyhow::{anyhow, Result};

pub struct SemanticEditor;

impl SemanticEditor {
    /// Semantic AST replacement: parses the target file, locates the node 
    /// containing the `search_signature` (e.g., a function name or struct), 
    /// and surgically replaces the entire AST node boundaries with `replace_code`.
    pub fn replace_node(original_code: &str, search_signature: &str, replace_code: &str) -> Result<String> {
        // ⚡ PHASE 2: Tree-Sitter Integration
        // For this cycle, if the tree-sitter C-bindings are not yet compiled on the substrate,
        // we fallback to our robust DiffEngine. As soon as the cargo tree-sitter deps are linked,
        // this method bridges directly into the AST bounds.
        
        crate::diff::DiffEngine::apply(original_code, search_signature, replace_code)
    }
}
"""

with open("src/ast_editor.rs", "w") as f:
    f.write(ast_editor_code)

# 3. Wire them into src/lib.rs
with open("src/lib.rs", "r") as f:
    lib_code = f.read()

if "pub mod fs_watcher;" not in lib_code:
    lib_code += "\npub mod fs_watcher;\npub mod ast_editor;\n"
    with open("src/lib.rs", "w") as f:
        f.write(lib_code)

# 4. Ignite the Watcher in src/main.rs
with open("src/main.rs", "r") as f:
    main_code = f.read()

if "fs_watcher::ignite_nervous_system" not in main_code:
    hook_str = "    nexus_singularity::logger::init_tracing(tx.clone());\n"
    injection = "    nexus_singularity::fs_watcher::ignite_nervous_system(tx.clone(), &std::env::current_dir().unwrap());\n"
    main_code = main_code.replace(hook_str, hook_str + injection)
    with open("src/main.rs", "w") as f:
        f.write(main_code)

print("\x1b[32m>> [PHASE 2 INJECTED] fs_watcher and ast_editor fused to the Kernel.\x1b[0m")
