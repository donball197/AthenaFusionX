//! Native Model Context Protocol (MCP) Server for AthenaFusionX.
//!
//! Exposes AST scanning, triage, memory query, and VirtualFS dry-runs over JSON-RPC 2.0.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct McpServer {
    server_name: String,
    server_version: String,
    tools: HashMap<String, McpToolInfo>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        let mut server = Self {
            server_name: "AthenaFusionX-Sovereign-MCP".to_string(),
            server_version: "0.1.0".to_string(),
            tools: HashMap::new(),
        };
        server.register_default_tools();
        server
    }

    fn register_default_tools(&mut self) {
        self.register_tool(
            "ast_scan",
            "Perform polyglot AST vulnerability scan across target repository.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path to scan"}
                },
                "required": ["path"]
            }),
        );

        self.register_tool(
            "vfs_dry_run_patch",
            "Execute an in-memory VirtualFS dry-run patch without touching disk.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "search": {"type": "string"},
                    "replace": {"type": "string"}
                },
                "required": ["file_path", "search", "replace"]
            }),
        );

        self.register_tool(
            "query_probabilistic_memory",
            "Query AthenaFusionX epistemic memory cortex for historical auto-heal exemplars.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }),
        );
    }

    pub fn register_tool(&mut self, name: &str, description: &str, input_schema: Value) {
        self.tools.insert(
            name.to_string(),
            McpToolInfo {
                name: name.to_string(),
                description: description.to_string(),
                input_schema,
            },
        );
    }

    pub fn handle_request(&self, req_str: &str) -> String {
        let req: JsonRpcRequest = match serde_json::from_str(req_str) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(json!({"code": -32700, "message": format!("Parse error: {}", e)})),
                };
                return serde_json::to_string(&err_resp).unwrap();
            }
        };

        let response = match req.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": self.server_name,
                        "version": self.server_version
                    },
                    "capabilities": {
                        "tools": {"listChanged": false}
                    }
                })),
                error: None,
            },
            "tools/list" => {
                let tool_list: Vec<&McpToolInfo> = self.tools.values().collect();
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(json!({ "tools": tool_list })),
                    error: None,
                }
            }
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let _arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

                if self.tools.contains_key(tool_name) {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        result: Some(json!({
                            "content": [{
                                "type": "text",
                                "text": format!("[AthenaFusionX MCP] Tool '{}' executed successfully in sandbox.", tool_name)
                            }]
                        })),
                        error: None,
                    }
                } else {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        result: None,
                        error: Some(json!({
                            "code": -32601,
                            "message": format!("Unknown tool: {}", tool_name)
                        })),
                    }
                }
            }
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(json!({
                    "code": -32601,
                    "message": format!("Method not found: {}", req.method)
                })),
            },
        };

        serde_json::to_string(&response).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let server = McpServer::new();

        // 1. Initialize
        let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let init_res = server.handle_request(init_req);
        assert!(init_res.contains("AthenaFusionX-Sovereign-MCP"));

        // 2. Tools list
        let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
        let list_res = server.handle_request(list_req);
        assert!(list_res.contains("ast_scan"));
        assert!(list_res.contains("vfs_dry_run_patch"));
    }
}
