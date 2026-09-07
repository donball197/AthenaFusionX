use nexus_runtime_core::mcp::McpServer;
use serde_json::{json, Value};

#[test]
fn test_mcp_e2e_lifecycle() {
    let server = McpServer::new();

    // 1. MCP Handshake: initialize
    let init_payload = json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "initialize",
        "params": {}
    })
    .to_string();

    let init_res_str = server.handle_request(&init_payload);
    let init_res: Value = serde_json::from_str(&init_res_str).expect("Valid JSON");

    assert_eq!(init_res["jsonrpc"], "2.0");
    assert_eq!(init_res["id"], 100);
    assert_eq!(init_res["result"]["serverInfo"]["name"], "AthenaFusionX-Sovereign-MCP");
    assert_eq!(init_res["result"]["protocolVersion"], "2024-11-05");

    // 2. Tool Discovery: tools/list
    let list_payload = json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "tools/list",
        "params": {}
    })
    .to_string();

    let list_res_str = server.handle_request(&list_payload);
    let list_res: Value = serde_json::from_str(&list_res_str).expect("Valid JSON");

    let tools = list_res["result"]["tools"].as_array().expect("tools array");
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert!(tool_names.contains(&"ast_scan"));
    assert!(tool_names.contains(&"vfs_dry_run_patch"));
    assert!(tool_names.contains(&"query_probabilistic_memory"));

    // 3. Tool Invocation: tools/call
    let call_payload = json!({
        "jsonrpc": "2.0",
        "id": 102,
        "method": "tools/call",
        "params": {
            "name": "ast_scan",
            "arguments": {
                "path": "nexus_audit_target"
            }
        }
    })
    .to_string();

    let call_res_str = server.handle_request(&call_payload);
    let call_res: Value = serde_json::from_str(&call_res_str).expect("Valid JSON");

    assert_eq!(call_res["id"], 102);
    assert!(call_res["result"]["content"][0]["text"].as_str().unwrap().contains("ast_scan"));
}
