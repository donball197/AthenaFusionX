//! Sandboxed filesystem tools exposed to the minion via function calling.
//! Read-only, path-traversal-safe, scoped to a workspace root.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves a user-supplied relative path against the workspace root,
/// refusing absolute paths and `..` traversal. Mirrors the safe_path()
/// pattern already used in bins/ide_backend/src/server.rs.
fn safe_path(workspace_root: &Path, user_path: &str) -> Result<PathBuf, String> {
    if user_path.is_empty() || user_path == "." {
        return Ok(workspace_root.to_path_buf());
    }
    let path = Path::new(user_path);
    if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("PATH_TRAVERSAL_DENIED".to_string());
    }
    Ok(workspace_root.join(path))
}

/// Returns the OpenAI-compatible `tools` schema array to send with each
/// chat completion request.
pub fn tool_schema() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List files and directories at a given path, relative to the repo root. Use '.' for the root.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path, e.g. 'crates/nexus_dag_orchestrator/src'" }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the full contents of a text file, relative to the repo root.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative file path, e.g. 'Cargo.toml'" }
                    },
                    "required": ["path"]
                }
            }
        }
    ])
}

/// Executes a tool call by name against the sandboxed workspace root.
/// Returns a JSON-serializable result (or an error string) as plain text,
/// suitable for a `tool` role message.
pub fn execute_tool(workspace_root: &Path, name: &str, arguments: &Value) -> String {
    match name {
        "list_dir" => {
            let rel = arguments["path"].as_str().unwrap_or(".");
            match safe_path(workspace_root, rel) {
                Err(e) => json!({ "error": e }).to_string(),
                Ok(target) => match fs::read_dir(&target) {
                    Ok(entries) => {
                        let names: Vec<Value> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                json!({
                                    "name": e.file_name().to_string_lossy(),
                                    "is_dir": is_dir
                                })
                            })
                            .collect();
                        json!({ "entries": names }).to_string()
                    }
                    Err(e) => json!({ "error": format!("{}", e) }).to_string(),
                },
            }
        }
        "read_file" => {
            let rel = arguments["path"].as_str().unwrap_or("");
            match safe_path(workspace_root, rel) {
                Err(e) => json!({ "error": e }).to_string(),
                Ok(target) => match fs::read_to_string(&target) {
                    Ok(content) => {
                        const MAX_LEN: usize = 8000;
                        if content.len() > MAX_LEN {
                            json!({ "content": format!("{}... [TRUNCATED]", &content[..MAX_LEN]) })
                                .to_string()
                        } else {
                            json!({ "content": content }).to_string()
                        }
                    }
                    Err(e) => json!({ "error": format!("{}", e) }).to_string(),
                },
            }
        }
        other => json!({ "error": format!("Unknown tool: {}", other) }).to_string(),
    }
}
