import os

# ==============================================================================
# 1. THE SEMANTIC ENGINE (src/ast_editor.rs)
# ==============================================================================
ast_code = r"""use tree_sitter::{Parser, Query, QueryCursor};
use std::fs;
use anyhow::{anyhow, Result};

pub struct AstEditor;

impl AstEditor {
    /// update_function: Finds a function by name and replaces its entire body.
    /// This is the "Semantic Precision" upgrade to replace simple string diffs.
    pub fn update_function(path: &str, function_name: &str, new_body: &str) -> Result<String> {
        let source_code = fs::read_to_string(path)?;
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        
        let tree = parser.parse(&source_code, None)
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
                if capture.index == 1 { // The @func capture
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
"""
with open("src/ast_editor.rs", "w") as f: f.write(ast_code)
print("\x1b[32m>> [SUTURE 1] src/ast_editor.rs (Semantic Engine) deployed.\x1b[0m")

# ==============================================================================
# 2. THE REFLEX ENGINE (src/fs_watcher.rs - Warp DNA)
# ==============================================================================
watcher_code = r"""use notify_debouncer_full::{new_debouncer, notify::*, DebounceEventResult};
use std::time::Duration;
use tracing::{info, warn, debug};
use tokio::sync::broadcast;

pub struct NexusWatcher;

impl NexusWatcher {
    /// ignite: Starts the OS-level event loop with Warp-style debouncing.
    pub fn ignite(tx: broadcast::Sender<String>) -> Result<Debouncer<RecommendedWatcher, NoCache>, Error> {
        let tx_clone = tx.clone();
        
        let mut debouncer = new_debouncer(Duration::from_millis(200), None, move |res: DebounceEventResult| {
            match res {
                Ok(events) => {
                    for event in events {
                        let paths: Vec<String> = event.event.paths.iter()
                            .map(|p| p.to_string_lossy().to_string()).collect();
                        
                        let msg = match event.event.kind {
                            EventKind::Modify(_) => format!("FS_MODIFIED: {:?}", paths),
                            EventKind::Create(_) => format!("FS_CREATED: {:?}", paths),
                            EventKind::Remove(_) => format!("FS_DELETED: {:?}", paths),
                            _ => continue,
                        };
                        
                        let _ = tx_clone.send(serde_json::json!({
                            "type": "LOG_EVENT",
                            "payload": msg
                        }).to_string());
                        debug!("{}", msg);
                    }
                }
                Err(e) => warn!("FS_WATCHER_ERROR: {:?}", e),
            }
        })?;

        debouncer.watcher().watch(std::path::Path::new("./src"), RecursiveMode::Recursive)?;
        info!("\x1b[36m>> [AUTONOMIC_WATCHER] OS-Level Hooks Active (Debounced)\x1b[0m");
        Ok(debouncer)
    }
}
"""
with open("src/fs_watcher.rs", "w") as f: f.write(watcher_code)
print("\x1b[32m>> [SUTURE 2] src/fs_watcher.rs (Reflex Engine) deployed.\x1b[0m")

# ==============================================================================
# 3. PROTOCOL ASCENSION (src/agent_protocol.rs)
# ==============================================================================
protocol_path = "src/agent_protocol.rs"
with open(protocol_path, "r") as f:
    code = f.read()

new_action = '    #[serde(rename = "patch_function")] PatchFunction { path: String, function_name: String, new_body: String, reasoning: String },\n'
if "patch_function" not in code:
    code = code.replace('    #[serde(rename = "halt")] Halt { reasoning: String },', 
                        new_action + '    #[serde(rename = "halt")] Halt { reasoning: String },')
    with open(protocol_path, "w") as f: f.write(code)
    print("\x1b[32m>> [SUTURE 3] AgentProtocol upgraded with patch_function.\x1b[0m")

# ==============================================================================
# 4. EXECUTOR SUTURE (src/executor.rs)
# ==============================================================================
exec_path = "src/executor.rs"
if os.path.exists(exec_path):
    with open(exec_path, "r") as f:
        code = f.read()
    
    if "AgentAction::PatchFunction" not in code:
        injection = r"""            AgentAction::PatchFunction { path, function_name, new_body, reasoning } => {
                let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": format!("Patching function {} in {} | Reason: {}", function_name, path, reasoning)}).to_string());
                match crate::ast_editor::AstEditor::update_function(&path, &function_name, &new_body) {
                    Ok(full_code) => {
                        match tokio::fs::write(&path, full_code).await {
                            Ok(_) => Ok(format!("[PATCH_OK] Function '{}' mutated in {}", function_name, path)),
                            Err(e) => Ok(format!("PATCH_WRITE_FAILED: {}", e)),
                        }
                    },
                    Err(e) => Ok(format!("PATCH_FAILED: {}", e)),
                }
            }
"""
        code = code.replace("            AgentAction::Halt { reasoning } => {", injection + "            AgentAction::Halt { reasoning } => {")
        with open(exec_path, "w") as f: f.write(code)
        print("\x1b[32m>> [SUTURE 4] RoboticExecutor upgraded for Semantic Patching.\x1b[0m")
