//! Native tool function declarations for AgentAction variants.
//! Compatible with both Gemini (functionDeclarations) and OpenAI/NVIDIA (tools) formats.

use crate::agent_protocol::AgentAction;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Function declaration shared across both APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl FunctionDeclaration {
    pub fn for_action(
        action_type: &str,
        description: &str,
        required_fields: &[&str],
        properties: serde_json::Value,
    ) -> Self {
        Self {
            name: action_type.to_string(),
            description: description.to_string(),
            parameters: json!({
                "type": "object",
                "properties": properties,
                "required": required_fields,
            }),
        }
    }
}

/// Returns the full list of tool declarations for all core AgentAction variants.
pub fn all_tool_declarations() -> Vec<FunctionDeclaration> {
    vec![
        FunctionDeclaration::for_action(
            "execute_command",
            "Execute a shell command in the sandboxed environment.",
            &["command", "reasoning"],
            json!({
                "command": { "type": "string", "description": "The bash command to execute." },
                "reasoning": { "type": "string", "description": "Why this command is necessary." },
            }),
        ),
        FunctionDeclaration::for_action(
            "read_file",
            "Read the full content of a file.",
            &["path", "reasoning"],
            json!({
                "path": { "type": "string", "description": "Path to the file relative to workspace root." },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "read_file_chunk",
            "Read a specific chunk of a file by line numbers.",
            &["path", "start_line", "max_lines", "reasoning"],
            json!({
                "path": { "type": "string" },
                "start_line": { "type": "integer", "minimum": 0 },
                "max_lines": { "type": "integer", "minimum": 1 },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "search_code",
            "Search for a pattern in source code files.",
            &["path", "query", "reasoning"],
            json!({
                "path": { "type": "string" },
                "query": { "type": "string", "description": "Search pattern or keyword." },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "write_file",
            "Write content to a file (overwrites if exists).",
            &["path", "content", "reasoning"],
            json!({
                "path": { "type": "string" },
                "content": { "type": "string", "description": "Full new content of the file." },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "diff_patch",
            "Apply a surgical string replacement patch to a file.",
            &["path", "search", "replace", "reasoning"],
            json!({
                "path": { "type": "string" },
                "search": { "type": "string", "description": "Exact text snippet to find." },
                "replace": { "type": "string", "description": "Exact replacement text." },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "patch_function",
            "Replace an entire Rust function body using AST surgery.",
            &["path", "function_name", "new_body", "reasoning"],
            json!({
                "path": { "type": "string" },
                "function_name": { "type": "string" },
                "new_body": { "type": "string", "description": "Complete new function definition." },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "run_tests",
            "Run cargo tests for a specific target package or module.",
            &["test_target", "reasoning"],
            json!({
                "test_target": { "type": "string", "description": "Crate package or test name." },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "search_memory",
            "Query the vector memory palace for historical context.",
            &["query", "reasoning"],
            json!({
                "query": { "type": "string", "description": "Natural language search query." },
                "reasoning": { "type": "string" },
            }),
        ),
        FunctionDeclaration::for_action(
            "halt",
            "Halt the current cognitive task when the mission is successfully completed.",
            &["reasoning"],
            json!({
                "reasoning": { "type": "string", "description": "Summary of completed objective." },
            }),
        ),
    ]
}

/// Fully maps tool names and argument objects back into native AgentAction variants.
pub fn parse_tool_call(tool_name: &str, args: &serde_json::Value) -> Result<AgentAction, String> {
    let get_str = |field: &str| -> Result<String, String> {
        args.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Missing or invalid field '{}' in tool call args", field))
    };

    let get_int = |field: &str| -> Result<usize, String> {
        args.get(field)
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .ok_or_else(|| format!("Missing or invalid integer field '{}'", field))
    };

    match tool_name {
        "execute_command" => Ok(AgentAction::ExecuteCommand {
            command: get_str("command")?,
            reasoning: get_str("reasoning")?,
            confidence: 0.95,
        }),
        "read_file" => Ok(AgentAction::ReadFile {
            path: get_str("path")?,
            reasoning: get_str("reasoning")?,
        }),
        "read_file_chunk" => Ok(AgentAction::ReadFileChunk {
            path: get_str("path")?,
            start_line: get_int("start_line")?,
            max_lines: get_int("max_lines")?,
            reasoning: get_str("reasoning")?,
        }),
        "search_code" => Ok(AgentAction::SearchCode {
            path: get_str("path")?,
            query: get_str("query")?,
            reasoning: get_str("reasoning")?,
        }),
        "write_file" => Ok(AgentAction::WriteFile {
            path: get_str("path")?,
            content: get_str("content")?,
            reasoning: get_str("reasoning")?,
            confidence: 0.95,
        }),
        "diff_patch" => Ok(AgentAction::DiffPatch {
            path: get_str("path")?,
            search: get_str("search")?,
            replace: get_str("replace")?,
            reasoning: get_str("reasoning")?,
        }),
        "patch_function" => Ok(AgentAction::PatchFunction {
            path: get_str("path")?,
            function_name: get_str("function_name")?,
            new_body: get_str("new_body")?,
            reasoning: get_str("reasoning")?,
        }),
        "run_tests" => Ok(AgentAction::RunTests {
            test_target: get_str("test_target")?,
            reasoning: get_str("reasoning")?,
        }),
        "search_memory" => Ok(AgentAction::SearchMemory {
            query: get_str("query")?,
            reasoning: get_str("reasoning")?,
        }),
        "halt" => Ok(AgentAction::Halt {
            reasoning: get_str("reasoning")?,
        }),
        other => Err(format!("Unrecognized tool identifier received: {}", other)),
    }
}

/// Universal response inspector that extracts tool calls from either Gemini or OpenAI/NVIDIA NIM responses.
pub fn extract_tool_call_from_response(response_json: &serde_json::Value) -> Result<AgentAction, String> {
    // 1. Check Gemini format: candidates[0].content.parts[0].functionCall
    if let Some(fc) = response_json.pointer("/candidates/0/content/parts/0/functionCall") {
        let name = fc["name"].as_str().ok_or("Gemini tool call missing 'name'")?;
        let args = fc.get("args").cloned().unwrap_or_else(|| json!({}));
        return parse_tool_call(name, &args);
    }

    // 2. Check OpenAI / NVIDIA NIM format: choices[0].message.tool_calls[0]
    if let Some(tc) = response_json.pointer("/choices/0/message/tool_calls/0") {
        let name = tc.pointer("/function/name").and_then(|v| v.as_str()).ok_or("OpenAI tool call missing function name")?;
        let args_str = tc.pointer("/function/arguments").and_then(|v| v.as_str()).unwrap_or("{}");
        let args: serde_json::Value = serde_json::from_str(args_str).map_err(|e| format!("Failed to parse tool arguments JSON string: {}", e))?;
        return parse_tool_call(name, &args);
    }

    Err("No native tool call found in model response payload.".to_string())
}
