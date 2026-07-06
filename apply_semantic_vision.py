import os

# --- 1. UPDATE AST EDITOR ---
ast_path = "crates/runtime_core/src/ast_editor.rs"
with open(ast_path, "r") as f:
    ast_data = f.read()

new_read_func = r"""    pub fn read_function(path: &str, function_name: &str) -> Result<String> {
        let source_code = fs::read_to_string(path)?;
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::language())?;

        let tree = parser
            .parse(&source_code, None)
            .ok_or_else(|| anyhow!("AST_PARSE_FAIL: Could not parse {}", path))?;

        let query_str = format!(
            "(function_item name: (identifier) @name (#eq? @name \"{}\")) @func",
            function_name
        );
        let query = Query::new(&tree_sitter_rust::language(), &query_str)?;
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source_code.as_bytes());
        for m in matches {
            for capture in m.captures {
                if capture.index == 1 {
                    let node = capture.node;
                    let start = node.start_byte();
                    let end = node.end_byte();
                    return Ok(source_code[start..end].to_string());
                }
            }
        }

        Err(anyhow!("FUNCTION_NOT_FOUND: Could not find function '{}' in {}", function_name, path))
    }

    pub fn update_function"""

ast_data = ast_data.replace('    pub fn update_function', new_read_func)
with open(ast_path, "w") as f:
    f.write(ast_data)


# --- 2. UPDATE AGENT PROTOCOL ---
proto_path = "crates/runtime_core/src/agent_protocol.rs"
with open(proto_path, "r") as f:
    proto_data = f.read()

new_proto = r"""    #[serde(rename = "read_function")]
    ReadFunction {
        path: String,
        function_name: String,
        #[serde(default)] reasoning: String,
    },
    #[serde(rename = "read_file")]"""

proto_data = proto_data.replace('    #[serde(rename = "read_file")]', new_proto)
with open(proto_path, "w") as f:
    f.write(proto_data)


# --- 3. UPDATE EXECUTOR ---
exec_path = "crates/runtime_core/src/executor.rs"
with open(exec_path, "r") as f:
    exec_data = f.read()

new_robotic_exec = r"""            AgentAction::ReadFunction { path, function_name, .. } => {
                let safe_path = sanitize(&path)?;
                let content = crate::ast_editor::AstEditor::read_function(safe_path.to_str().unwrap_or(&path), &function_name)?;
                Ok(format!("[READ_FUNCTION_OK] {} in {}\nCONTENT:\n{}", function_name, path, content))
            }
            AgentAction::ReadFile { path, .. } => {"""

new_tool_exec = r"""            AgentAction::ReadFunction { path, function_name, .. } => {
                let content = crate::ast_editor::AstEditor::read_function(path, function_name)?;
                Ok(format!("[READ_FUNCTION_OK] {} in {}\nCONTENT:\n{}", function_name, path, content))
            }
            AgentAction::ReadFile { path, .. } => {"""

# Replace in RoboticExecutor (first instance)
exec_data = exec_data.replace(
    '            AgentAction::ReadFile { path, .. } => {',
    new_robotic_exec,
    1
)
# Replace in ToolExecutor (second instance)
exec_data = exec_data.replace(
    '            AgentAction::ReadFile { path, .. } => {',
    new_tool_exec,
    1
)
with open(exec_path, "w") as f:
    f.write(exec_data)


# --- 4. UPDATE ROUTER PROMPT ---
router_path = "crates/runtime_core/src/router.rs"
with open(router_path, "r") as f:
    router_data = f.read()

router_data = router_data.replace(
    '{ "type": "read_file", "path": "...", "reasoning": "..." },',
    '{ "type": "read_function", "path": "...", "function_name": "...", "reasoning": "..." },\n  { "type": "read_file", "path": "...", "reasoning": "..." },'
)

new_rules = r"""CRITICAL RULES:
1. NEVER use `write_file` to modify an existing function. You MUST use `patch_function` or `diff_patch`.
2. ALWAYS prefer `read_function` over `read_file` if you only need to inspect a specific function. This dramatically saves context space."""

router_data = router_data.replace(
    'CRITICAL RULES:\n1. NEVER use `write_file` to modify an existing function. You MUST use `patch_function` or `diff_patch`.',
    new_rules
)
with open(router_path, "w") as f:
    f.write(router_data)


# --- 5. UPDATE EPISTEMIC ENGINE ---
ee_path = "crates/runtime_core/src/epistemic_engine.rs"
with open(ee_path, "r") as f:
    ee_data = f.read()

ee_data = ee_data.replace(
    '2. [{ "type": "read_file"',
    '2. [{ "type": "read_function", "path": "<filepath>", "function_name": "<name>", "reasoning": "..." }]\n             2b. [{ "type": "read_file"'
)

ee_data = ee_data.replace(
    'if evidence.contains("[READ_OK]") {',
    'if evidence.contains("[READ_OK]") || evidence.contains("[READ_FUNCTION_OK]") {'
)
with open(ee_path, "w") as f:
    f.write(ee_data)

print("[✓] AST Semantic Vision (Phase 2) applied successfully.")
