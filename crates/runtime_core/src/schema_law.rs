pub const SYSTEM_JSON_SCHEMA: &str = r#"MANDATORY SCHEMA:
[
  { "type": "execute_command", "command": "...", "reasoning": "..." },
  { "type": "read_function", "path": "...", "function_name": "...", "reasoning": "..." },
  { "type": "read_file", "path": "...", "reasoning": "..." },
  { "type": "write_file", "path": "...", "content": "...", "reasoning": "..." },
  { "type": "diff_patch", "path": "...", "search": "...", "replace": "...", "reasoning": "..." },
  { "type": "patch_file", "path": "...", "target": "...", "replacement": "...", "reasoning": "..." },
  { "type": "patch_function", "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." },
  { "type": "search_memory", "query": "...", "reasoning": "..." },
  { "type": "vmo_create", "size": 1024, "reasoning": "..." },
  { "type": "calculate_entropy", "handle": "...", "reasoning": "..." },
  { "type": "lazarus_snapshot", "path": "...", "tag": "optional", "reasoning": "..." },
  { "type": "export_report", "title": "...", "content": "...", "reasoning": "..." },
  { "type": "fetch_ui_snapshot", "reasoning": "..." },
  { "type": "scrape_url", "url": "...", "reasoning": "..." },
  { "type": "spawn_subagent", "instruction": "...", "tier": "Specialist", "limit": 5, "reasoning": "..." },
  { "type": "delegate_task", "instruction": "...", "tier": "Specialist", "token_limit": 1000, "reasoning": "..." },
  { "type": "git_rollback", "reasoning": "..." },
  { "type": "halt", "reasoning": "..." }
]

CRITICAL RULES:
1. NEVER use `write_file` to modify an existing function. You MUST use `patch_function` or `diff_patch`.
2. `patch_function` requires `path`, `function_name`, and the full replacement code in `new_body`.
3. `diff_patch` requires `search` (the exact old string) and `replace` (the exact new string). DO NOT output unified diffs or use a `diff` field.
4. When you have achieved the task objective, include the `halt` action in your JSON array.
5. You MUST escape all newlines inside JSON strings as `
` (e.g. "line1\nline2").
6. NOTE: Do NOT use types like 'find_file', 'ls_tree', or 'grep_code'. If you need to search or list, use 'execute_command' (e.g. command: 'ls -F' or command: 'find .')."#;
